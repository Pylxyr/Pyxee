//! AAC-LC in M4A is the format the app asks YouTube for (see `FORMAT_SELECTOR`), and YouTube serves
//! it fragmented: `moov`, `sidx`, then `moof`+`mdat` pairs. These tests play that layout through
//! the real reader, from a file and over HTTP range requests, including seeking.
//!
//! The `*_frag.m4a` fixtures were made once from `a_44k.m4a` / `b_48k.m4a` by stream copy (no
//! re-encode) with an MP4 muxer's dash/fragmented mode: 500 ms fragments, empty moov, global sidx.

use pyxee_audio::decode::{LibavSource, SampleSource};
use pyxee_audio::http::HttpRangeSource;
mod common;
use common::range_server;
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

fn open_file(name: &str) -> LibavSource {
    LibavSource::open_path(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
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
        LibavSource::open(Box::new(h)).expect("open over http")
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
