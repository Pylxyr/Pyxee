//! Opus is the format YouTube serves best (WebM) and what local `.opus` files use. These tests
//! cover what the gapless tests do not: seeking in place (the decoder needs pre-roll after a
//! seek) and streaming over HTTP range requests, the real YouTube path.

mod common;

use common::range_server;
use pyxee_audio::decode::{LibavSource, SampleSource};
use pyxee_audio::http::HttpRangeSource;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn read_all(s: &mut LibavSource) -> Vec<f32> {
    let mut all = Vec::new();
    while s.read(&mut all).expect("read") {}
    all
}

/// Largest sample difference between `part` and the same stretch of `full` starting at `frame`.
fn max_diff(full: &[f32], part: &[f32], frame: usize) -> f32 {
    let base = frame * 2;
    part.iter()
        .zip(&full[base..])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max)
}

/// `tolerance` is in frames: Matroska timestamps are whole milliseconds (48 frames at 48 kHz), Ogg's are exact.
fn check_seek(name: &str, tolerance: usize, mut straight: LibavSource, mut seeked: LibavSource) {
    let full = read_all(&mut straight);
    assert_eq!(full.len() / 2, 96_000, "{name}: full length");
    seeked.seek_ms(1000).expect("seek");
    let part = read_all(&mut seeked);
    let frames = part.len() / 2;
    println!("[{name}] seeked frames={frames} (expected 48000)");
    assert!(
        frames.abs_diff(48_000) <= tolerance,
        "{name}: {frames} frames after seek"
    );
    // The tail must be the same audio as a straight decode, from the very first frame: the
    // lead-in dropped after the seek is what makes the landing sample-accurate.
    let d = max_diff(&full, &part, 96_000 - frames);
    println!("[{name}] max sample difference vs straight decode = {d}");
    assert!(
        d < 1e-3,
        "{name}: seeked audio differs from a straight decode by {d}"
    );
}

#[test]
fn opus_seeks_in_place_to_the_same_audio_in_ogg_and_webm() {
    for (name, tolerance) in [("a_48k.opus", 1), ("a_48k.webm", 48)] {
        let open =
            || LibavSource::open_path(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        check_seek(name, tolerance, open(), open());
    }
}

#[test]
fn opus_webm_plays_and_seeks_over_http_range_requests() {
    let url = range_server(std::fs::read(fixture("a_48k.webm")).unwrap());
    let open = || {
        let h = HttpRangeSource::open(&url).expect("range source");
        LibavSource::open(Box::new(h)).expect("open over http")
    };
    check_seek("webm-http", 48, open(), open());
}
