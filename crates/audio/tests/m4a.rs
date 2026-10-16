//! AAC-LC in M4A is the format the app asks YouTube for (see `FORMAT_SELECTOR`), and YouTube serves
//! it fragmented: `moov`, `sidx`, then `moof`+`mdat` pairs. These tests play that layout through
//! the real reader, from a file and over HTTP range requests, including seeking.
//!
//! The `*_frag.m4a` fixtures were made once from `a_44k.m4a` / `b_48k.m4a` by stream copy (no
//! re-encode) with an MP4 muxer's dash/fragmented mode: 500 ms fragments, empty moov, global sidx.

use pyxee_audio::decode::{SampleSource, SymphoniaSource};
use pyxee_audio::http::HttpRangeSource;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn frames_left(s: &mut dyn SampleSource) -> usize {
    let (mut n, mut buf) = (0, Vec::new());
    loop {
        buf.clear();
        if !s.read(&mut buf).expect("read") {
            return n;
        }
        n += buf.len() / s.channels();
    }
}

fn secs(frames: usize, s: &dyn SampleSource) -> f64 {
    frames as f64 / f64::from(s.sample_rate())
}

fn open_file(name: &str) -> SymphoniaSource {
    SymphoniaSource::open_path(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Serves `body` with range support, like googlevideo does for these streams.
fn range_server(body: Vec<u8>) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let body = body.clone();
            std::thread::spawn(move || {
                let (mut req, mut b) = (Vec::new(), [0u8; 1]);
                while !req.ends_with(b"\r\n\r\n") {
                    if s.read(&mut b).unwrap_or(0) == 0 {
                        return;
                    }
                    req.push(b[0]);
                }
                let req = String::from_utf8_lossy(&req).to_string();
                let spec = req
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("range: bytes=")
                            .map(str::to_string)
                    })
                    .unwrap_or_else(|| "0-".into());
                let (a, z) = spec.split_once('-').unwrap();
                let from: usize = a.trim().parse().unwrap_or(0);
                let to = z
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
    format!("http://{addr}/audio")
}

#[test]
fn a_fragmented_m4a_plays_to_its_full_length() {
    let mut s = open_file("a_44k_frag.m4a");
    let n = frames_left(&mut s);
    assert!((secs(n, &s) - 2.0).abs() < 0.01, "{:.3} s", secs(n, &s));
}

#[test]
fn seeking_a_fragmented_m4a_lands_at_the_offset() {
    let mut s = open_file("b_48k_frag.m4a");
    s.seek_ms(1000).expect("seek");
    let n = frames_left(&mut s);
    // About one AAC frame (23 ms) early at most: the decoder's priming, same as for a plain m4a.
    assert!(
        (secs(n, &s) - 1.0).abs() < 0.03,
        "{:.3} s left",
        secs(n, &s)
    );
}

#[test]
fn a_fragmented_m4a_plays_and_seeks_over_http_range_requests() {
    let url = range_server(std::fs::read(fixture("a_44k_frag.m4a")).unwrap());
    let open = || {
        let h = HttpRangeSource::open(&url).expect("range source");
        SymphoniaSource::open(Box::new(h), Some("m4a")).expect("open over http")
    };
    let mut s = open();
    let n = frames_left(&mut s);
    assert!((secs(n, &s) - 2.0).abs() < 0.01, "{:.3} s", secs(n, &s));
    let mut s = open();
    s.seek_ms(1500).expect("seek over http");
    let n = frames_left(&mut s);
    assert!(
        (secs(n, &s) - 0.5).abs() < 0.03,
        "{:.3} s left",
        secs(n, &s)
    );
}

#[test]
fn decoding_through_an_offset_agrees_with_a_real_seek() {
    // `skip_ms` is the fallback for a stream that cannot seek in place: it must end up in the same place.
    for name in [
        "a_44k.m4a",
        "a_44k_frag.m4a",
        "a_44k.mp3",
        "a_44k.flac",
        "b_48k.wav",
    ] {
        let mut sought = open_file(name);
        sought.seek_ms(1000).expect("seek");
        let a = frames_left(&mut sought);
        let mut skipped = open_file(name);
        skipped.skip_ms(1000);
        let b = frames_left(&mut skipped);
        let diff = secs(a.abs_diff(b), &sought);
        assert!(
            diff < 0.03,
            "{name}: seek left {a} frames, skip left {b} ({diff:.3} s apart)"
        );
    }
}
