//! YouTube answers 403 unless the request carries the headers yt-dlp used (User-Agent etc.).
//! These tests use a local server that behaves the same way.

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
