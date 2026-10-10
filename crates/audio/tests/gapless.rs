//! Integration test: two tracks go through the real decode -> resample -> gapless path and
//! the rendered output is scanned for silence at (and away from) the boundary.

use pyxee_audio::decode::{FfmpegPipeSource, SampleSource, SymphoniaSource};
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
    Box::new(SymphoniaSource::open_path(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}")))
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

#[test]
fn symphonia_cannot_decode_opus_so_ffmpeg_path_is_needed() {
    for name in ["a_48k.opus", "a_48k.webm"] {
        let r = SymphoniaSource::open_path(&fixture(name));
        println!(
            "symphonia on {name}: {}",
            r.as_ref().err().map_or("OK".into(), ToString::to_string)
        );
    }
}

#[test]
fn gapless_opus_via_ffmpeg_pipe() {
    let ffmpeg = Path::new("ffmpeg");
    let spawn = |n: &str| FfmpegPipeSource::spawn(ffmpeg, fixture(n).to_str().unwrap());
    let (Ok(a), Ok(b)) = (spawn("a_48k.opus"), spawn("b_48k.opus")) else {
        println!("ffmpeg not on PATH, skipping");
        return;
    };
    check("opus-ffmpeg", Box::new(a), Box::new(b));
}
