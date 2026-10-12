//! Failure modes must degrade gracefully: clear errors, no panics, no silent skips.

use pyxee_audio::decode::{SampleSource, SymphoniaSource};
use pyxee_audio::gapless::{Event, GaplessSource, PreparedTrack};
use pyxee_audio::http::HttpRangeSource;
use pyxee_audio::AudioError;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn tmp(name: &str, bytes: &[u8]) -> PathBuf {
    let p = std::env::temp_dir().join(format!("pyxee-test-{}-{name}", std::process::id()));
    std::fs::write(&p, bytes).unwrap();
    p
}

fn drain(g: &mut GaplessSource) -> Vec<f32> {
    let mut all = Vec::new();
    let mut buf = vec![0f32; 4096];
    loop {
        let n = g.fill(&mut buf);
        all.extend_from_slice(&buf[..n]);
        if n < buf.len() {
            return all;
        }
    }
}

struct Fake {
    mono: bool,
    left: usize,
}

impl SampleSource for Fake {
    fn sample_rate(&self) -> u32 {
        48_000
    }
    fn channels(&self) -> usize {
        if self.mono {
            1
        } else {
            2
        }
    }
    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError> {
        if self.left == 0 {
            return Ok(false);
        }
        let n = self.left.min(1000);
        self.left -= n;
        for i in 0..n {
            let v = (i as f32 * 0.1).sin() * 0.5 + 0.1;
            out.push(v);
            if !self.mono {
                out.push(v);
            }
        }
        Ok(true)
    }
}

#[test]
fn garbage_file_gives_a_clear_error() {
    let p = tmp("garbage.mp3", &[0x13, 0x37, 0x42, 0x99].repeat(5000));
    let err = SymphoniaSource::open_path(&p)
        .err()
        .expect("garbage must not open");
    println!("{err}");
    assert!(matches!(err, AudioError::Unsupported(_)), "{err:?}");
}

#[test]
fn truncated_file_plays_what_it_has() {
    let full = std::fs::read(fixture("a_44k.mp3")).unwrap();
    let p = tmp("trunc.mp3", &full[..full.len() * 4 / 10]);
    let mut g = GaplessSource::new();
    g.push(
        PreparedTrack::new(1, Box::new(SymphoniaSource::open_path(&p).unwrap()), 48_000).unwrap(),
    );
    let frames = drain(&mut g).len() / 2;
    println!("truncated mp3 decoded {frames} frames");
    assert!(frames > 10_000 && frames < 96_000);
}

#[test]
fn empty_track_is_reported_as_failed_not_skipped() {
    let mut g = GaplessSource::new();
    g.push(
        PreparedTrack::new(
            1,
            Box::new(Fake {
                mono: false,
                left: 0,
            }),
            48_000,
        )
        .unwrap(),
    );
    g.push(
        PreparedTrack::new(
            2,
            Box::new(Fake {
                mono: false,
                left: 5000,
            }),
            48_000,
        )
        .unwrap(),
    );
    let frames = drain(&mut g).len() / 2;
    let ev = g.drain_events();
    assert_eq!(frames, 5000);
    assert!(
        ev.iter().any(|e| matches!(e, Event::Failed { id: 1, .. })),
        "{ev:?}"
    );
    assert!(
        ev.iter().any(|e| matches!(
            e,
            Event::Finished {
                id: 2,
                frames_emitted: 5000,
                ..
            }
        )),
        "{ev:?}"
    );
}

#[test]
fn mono_is_upmixed_to_identical_stereo() {
    let mut g = GaplessSource::new();
    g.push(
        PreparedTrack::new(
            1,
            Box::new(Fake {
                mono: true,
                left: 3000,
            }),
            48_000,
        )
        .unwrap(),
    );
    let out = drain(&mut g);
    assert_eq!(out.len(), 6000);
    assert!(out.chunks(2).all(|f| f[0] == f[1]));
}

#[test]
fn three_tracks_join_without_losing_frames() {
    let mut g = GaplessSource::new();
    for (id, n) in [(1u64, 7777usize), (2, 1), (3, 12345)] {
        g.push(
            PreparedTrack::new(
                id,
                Box::new(Fake {
                    mono: false,
                    left: n,
                }),
                48_000,
            )
            .unwrap(),
        );
    }
    assert_eq!(drain(&mut g).len() / 2, 7777 + 1 + 12345);
}

/// Minimal range-capable HTTP server (one request per connection). `forbid` makes every request 403.
fn serve(body: Vec<u8>, forbid: bool) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut req = Vec::new();
            let mut b = [0u8; 1];
            while !req.ends_with(b"\r\n\r\n") {
                if s.read(&mut b).unwrap_or(0) == 0 {
                    break;
                }
                req.push(b[0]);
            }
            let text = String::from_utf8_lossy(&req).to_string();
            if forbid {
                let _ = s.write_all(
                    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                continue;
            }
            let range = text
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("range: bytes=")
                        .map(str::to_string)
                })
                .unwrap_or_default();
            let (a, b) = range.split_once('-').unwrap_or(("0", "0"));
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
        }
    });
    format!("http://{addr}/track")
}

#[test]
fn streams_over_http_range_requests_like_a_local_file() {
    let bytes = std::fs::read(fixture("a_44k.flac")).unwrap();
    let url = serve(bytes, false);
    let src = HttpRangeSource::open(&url).unwrap();
    let mut g = GaplessSource::new();
    g.push(
        PreparedTrack::new(
            1,
            Box::new(SymphoniaSource::open(Box::new(src), Some("flac")).unwrap()),
            48_000,
        )
        .unwrap(),
    );
    assert_eq!(drain(&mut g).len() / 2, 96_000);
}

#[test]
fn rejected_url_is_reported_as_expired_so_it_can_be_re_resolved() {
    let url = serve(Vec::new(), true);
    let err = HttpRangeSource::open(&url).err().expect("403 must fail");
    assert!(matches!(err, AudioError::UrlExpired(403)), "{err:?}");
}

#[test]
fn a_server_without_range_support_is_reported_as_unusable_for_this_reader_not_as_being_offline() {
    // A plain file server answers 200 with the whole body and no Content-Range. The song is fine; this
    // reader just cannot use it, so the caller must be able to tell and try another way (ffmpeg).
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut b = [0u8; 1];
            let mut req = Vec::new();
            while !req.ends_with(b"\r\n\r\n") {
                if s.read(&mut b).unwrap_or(0) == 0 {
                    break;
                }
                req.push(b[0]);
            }
            let _ = s.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nabcd",
            );
        }
    });
    let err = HttpRangeSource::open(&format!("http://{addr}/x"))
        .err()
        .expect("must not open");
    assert!(matches!(err, AudioError::Unsupported(_)), "{err:?}");
}
