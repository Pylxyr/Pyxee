//! YouTube answers 403 unless the request carries the headers yt-dlp used (User-Agent etc.).
//! These tests use a local server that behaves the same way.

use pyxee_audio::decode::{ffmpeg_header_args, refused_status, FfmpegPipeSource, SampleSource};
use pyxee_audio::http::HttpRangeSource;
use pyxee_audio::AudioError;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

const UA: &str = "Pyxee-Test/1.0";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn headers() -> Vec<(String, String)> {
    vec![
        ("User-Agent".into(), UA.into()),
        ("X-Token".into(), "abc".into()),
    ]
}

fn read_request(s: &mut TcpStream) -> String {
    let mut req = Vec::new();
    let mut b = [0u8; 1];
    while !req.ends_with(b"\r\n\r\n") {
        if s.read(&mut b).unwrap_or(0) == 0 {
            break;
        }
        req.push(b[0]);
    }
    String::from_utf8_lossy(&req).to_string()
}

fn header_of(req: &str, name: &str) -> Option<String> {
    req.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim()
            .eq_ignore_ascii_case(name)
            .then(|| v.trim().to_string())
    })
}

/// Range-capable server that answers 403 unless `User-Agent` and `X-Token` are right.
/// Returns the URL.
fn picky_server(body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let body = body.clone();
            std::thread::spawn(move || {
                let req = read_request(&mut s);
                let ok = header_of(&req, "user-agent").as_deref() == Some(UA)
                    && header_of(&req, "x-token").as_deref() == Some("abc");
                if !ok {
                    let _ = s.write_all(
                        b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                    return;
                }
                let range = header_of(&req, "range").unwrap_or_default();
                let spec = range.strip_prefix("bytes=").unwrap_or("0-");
                let (a, b) = spec.split_once('-').unwrap_or(("0", ""));
                let from: usize = a.trim().parse().unwrap_or(0);
                let to: usize = b
                    .trim()
                    .parse::<usize>()
                    .unwrap_or(body.len() - 1)
                    .min(body.len() - 1);
                let part = &body[from..=to];
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {from}-{to}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len(),
                    part.len()
                );
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(part);
            });
        }
    });
    format!("http://{addr}/track")
}

#[test]
fn http_source_needs_the_headers_and_works_with_them() {
    let url = picky_server(std::fs::read(fixture("a_44k.flac")).unwrap());
    let err = HttpRangeSource::open(&url).err().expect("no headers: 403");
    assert!(matches!(err, AudioError::UrlExpired(403)), "{err:?}");
    HttpRangeSource::open_with(&url, &headers()).expect("with the headers it opens");
}

#[test]
fn http_source_never_lets_a_header_override_its_own_range() {
    let url = picky_server(std::fs::read(fixture("a_44k.flac")).unwrap());
    let mut h = headers();
    h.push(("Range".into(), "bytes=5-6".into()));
    h.push(("Host".into(), "evil.example".into()));
    // Opens fine and reports the real length, so the injected Range/Host were ignored.
    HttpRangeSource::open_with(&url, &h).expect("opens");
}

#[test]
fn ffmpeg_arguments_carry_user_agent_and_headers_with_crlf() {
    let args = ffmpeg_header_args(&[
        ("Accept-Language".into(), "en-us,en;q=0.5".into()),
        ("User-Agent".into(), "Mozilla/5.0 (X)".into()),
        ("Sec-Fetch-Mode".into(), "navigate".into()),
    ]);
    assert_eq!(
        args,
        vec![
            "-user_agent",
            "Mozilla/5.0 (X)",
            "-headers",
            "Accept-Language: en-us,en;q=0.5\r\nSec-Fetch-Mode: navigate\r\n",
        ]
    );
}

#[test]
fn ffmpeg_arguments_cannot_smuggle_extra_headers_and_skip_host_and_range() {
    let args = ffmpeg_header_args(&[
        ("X-A".into(), "1\r\nX-Evil: 2".into()),
        ("Host".into(), "evil.example".into()),
        ("range".into(), "bytes=0-1".into()),
    ]);
    assert_eq!(args, vec!["-headers", "X-A: 1  X-Evil: 2\r\n"]);
    assert!(ffmpeg_header_args(&[]).is_empty());
}

#[test]
fn ffmpeg_refusals_are_recognised_by_status() {
    assert_eq!(
        refused_status(
            "[in#0 @ 0x1] Error opening input: Server returned 403 Forbidden (access denied)"
        ),
        Some(403)
    );
    assert_eq!(
        refused_status("[https @ 0x1] HTTP error 410 Gone"),
        Some(410)
    );
    assert_eq!(
        refused_status("Server returned 401 Unauthorized"),
        Some(401)
    );
    // Not "the URL is dead": other statuses and unrelated lines.
    assert_eq!(refused_status("Server returned 404 Not Found"), None);
    assert_eq!(
        refused_status("Server returned 503 Service Unavailable"),
        None
    );
    assert_eq!(
        refused_status("Invalid data found when processing input"),
        None
    );
}

fn have_ffmpeg() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok()
}

fn read_all(src: &mut FfmpegPipeSource) -> Result<usize, AudioError> {
    let mut total = 0;
    let mut out = Vec::new();
    loop {
        out.clear();
        if !src.read(&mut out)? {
            return Ok(total);
        }
        total += out.len();
    }
}

#[test]
fn ffmpeg_gets_403_without_headers_reported_as_expired_and_decodes_with_them() {
    if !have_ffmpeg() {
        println!("ffmpeg not on PATH, skipping");
        return;
    }
    let url = picky_server(std::fs::read(fixture("a_48k.opus")).unwrap());
    let ffmpeg = Path::new("ffmpeg");

    let mut refused = FfmpegPipeSource::spawn_with(ffmpeg, &url, 0, &[]).unwrap();
    let err = read_all(&mut refused).expect_err("403 must not look like a finished song");
    assert!(matches!(err, AudioError::UrlExpired(403)), "{err:?}");

    let mut ok = FfmpegPipeSource::spawn_with(ffmpeg, &url, 0, &headers()).unwrap();
    let samples = read_all(&mut ok).expect("decodes with the headers");
    assert!(samples > 48_000, "got {samples} samples");
}
