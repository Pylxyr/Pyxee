//! Integration test: two tracks go through the real decode -> resample -> gapless path and
//! the rendered output is scanned for silence at (and away from) the boundary.

use pyxee_audio::decode::{LibavSource, SampleSource};
use pyxee_audio::gap::find_gaps;
use pyxee_audio::gapless::{Event, GaplessSource, PreparedTrack};
use std::path::{Path, PathBuf};

const OUT_RATE: u32 = 48_000;
const TRACK_FRAMES: usize = 2 * OUT_RATE as usize;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn render(a: Box<dyn SampleSource>, b: Box<dyn SampleSource>) -> (Vec<f32>, Vec<Event>) {
    let mut g = GaplessSource::new();
    g.push(PreparedTrack::new(1, a, OUT_RATE).unwrap());
    g.push(PreparedTrack::new(2, b, OUT_RATE).unwrap());
    let mut all = Vec::new();
    let mut buf = vec![0f32; 4096];
    loop {
        let n = g.fill(&mut buf);
        all.extend_from_slice(&buf[..n]);
        if n < buf.len() {
            break;
        }
    }
    (all, g.drain_events())
}

fn check(ext: &str, a: Box<dyn SampleSource>, b: Box<dyn SampleSource>) {
    let (out, events) = render(a, b);
    let frames = out.len() / 2;
    let gaps = find_gaps(&out, 2, 1e-3, 8);
    let lens: Vec<u64> = events
        .iter()
        .filter_map(|e| match e {
            Event::Finished { frames_emitted, .. } => Some(*frames_emitted),
            _ => None,
        })
        .collect();
    println!(
        "[{ext}] frames={frames} (expected {}) per-track={lens:?} gaps={gaps:?}",
        2 * TRACK_FRAMES
    );
    assert!(gaps.is_empty(), "[{ext}] silence found: {gaps:?}");
    assert_eq!(lens.len(), 2, "[{ext}] both tracks must finish: {events:?}");
    // Tracks must keep their true length (priming/padding trimmed): within 5 ms.
    for l in lens {
        let diff = (l as i64 - TRACK_FRAMES as i64).unsigned_abs();
        assert!(diff <= 240, "[{ext}] track length off by {diff} frames");
    }
}

fn open(name: &str) -> Box<dyn SampleSource> {
    Box::new(LibavSource::open_path(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}")))
}

#[test]
fn gapless_wav() {
    check("wav", open("a_44k.wav"), open("b_48k.wav"));
}
#[test]
fn gapless_flac() {
    check("flac", open("a_44k.flac"), open("b_48k.flac"));
}
#[test]
fn gapless_mp3_320() {
    check("mp3", open("a_44k.mp3"), open("b_48k.mp3"));
}
#[test]
fn gapless_aac_m4a() {
    check("m4a", open("a_44k.m4a"), open("b_48k.m4a"));
}
#[test]
fn gapless_vorbis() {
    check("ogg", open("a_44k.ogg"), open("b_48k.ogg"));
}

/// YouTube's AAC layout: `moov` first, then a `sidx`, then one `moof`+`mdat` per fragment (the
/// fixtures are the plain m4a files stream-copied into that layout).
#[test]
fn gapless_aac_m4a_fragmented_like_youtube() {
    check("m4a-frag", open("a_44k_frag.m4a"), open("b_48k_frag.m4a"));
}

/// Opus is YouTube's best audio and ships in WebM; Ogg Opus is what local `.opus` files use. Both must
/// decode with the encoder's pre-skip and end padding removed (no silence, true length).
#[test]
fn gapless_opus_ogg() {
    check("opus", open("a_48k.opus"), open("a_48k.opus"));
}
#[test]
fn gapless_opus_webm() {
    check("webm", open("a_48k.webm"), open("a_48k.webm"));
}
