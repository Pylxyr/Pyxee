//! Seeking inside the open source: the decoded-ahead audio is dropped, what is queued behind stays, the track is
//! announced as started again, and anything that cannot seek in place says so (the caller then re-opens it).

use pyxee_audio::decode::SampleSource;
use pyxee_audio::gapless::{Event, GaplessSource, PreparedTrack};
use pyxee_audio::pipeline::{PipeEvent, Pipeline, Volume};
use pyxee_audio::AudioError;
use pyxee_core::{EntryId, Heard};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

const RATE: u32 = 48_000;

/// Stereo at the output rate (so nothing is resampled): every frame's sample is its own index / 1e6, so a sample
/// says where in the track it came from.
struct Ramp {
    frame: u64,
    total: u64,
    seekable: bool,
}

impl Ramp {
    fn new(total_ms: u64, seekable: bool) -> Self {
        Self {
            frame: 0,
            total: total_ms * u64::from(RATE) / 1000,
            seekable,
        }
    }
}

impl SampleSource for Ramp {
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn channels(&self) -> usize {
        2
    }
    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError> {
        if self.frame >= self.total {
            return Ok(false);
        }
        let n = 1024.min(self.total - self.frame);
        for f in self.frame..self.frame + n {
            let v = f as f32 / 1.0e6;
            out.extend([v, v]);
        }
        self.frame += n;
        Ok(true)
    }
    fn seek_ms(&mut self, ms: u64) -> Result<(), AudioError> {
        if !self.seekable {
            return Err(AudioError::Unsupported("no seeking here".into()));
        }
        self.frame = (ms * u64::from(RATE) / 1000).min(self.total);
        Ok(())
    }
}

fn frame_of(sample: f32) -> u64 {
    (sample * 1.0e6).round() as u64
}

fn track(id: u64, ms: u64, seekable: bool) -> PreparedTrack {
    PreparedTrack::new(id, Box::new(Ramp::new(ms, seekable)), RATE).unwrap()
}

#[test]
fn the_playing_track_continues_from_the_new_position_and_is_announced_again() {
    let mut g = GaplessSource::new();
    g.push(track(1, 10_000, true));
    let mut buf = vec![0f32; 2 * 4800];
    assert_eq!(g.fill(&mut buf), buf.len());
    assert_eq!(frame_of(buf[0]), 0);
    let _ = g.drain_events(); // Started{1, 0}

    g.seek_current(1, 4_000).unwrap();
    assert_eq!(g.fill(&mut buf), buf.len());
    assert_eq!(frame_of(buf[0]), 4_000 * 48, "continues at the target");
    assert_eq!(
        g.drain_events(),
        vec![Event::Started { id: 1, at_frame: 0 }],
        "the timeline starts again at the seek"
    );
}

#[test]
fn only_the_track_that_is_playing_can_be_sought_and_a_refusal_changes_nothing() {
    let mut g = GaplessSource::new();
    g.push(track(1, 10_000, true));
    g.push(track(2, 10_000, true));
    let mut buf = vec![0f32; 2 * 480];
    g.fill(&mut buf);
    let _ = g.drain_events();
    // Track 2 is queued, not playing; 9 does not exist.
    assert!(g.seek_current(2, 1_000).is_err());
    assert!(g.seek_current(9, 1_000).is_err());
    g.fill(&mut buf);
    assert_eq!(frame_of(buf[0]), 480, "playback went on from where it was");
    assert!(g.drain_events().is_empty());
}

#[test]
fn a_source_that_cannot_seek_in_place_refuses() {
    let mut g = GaplessSource::new();
    g.push(track(1, 10_000, false));
    let mut buf = vec![0f32; 2 * 480];
    g.fill(&mut buf);
    assert!(g.seek_current(1, 1_000).is_err());
}

fn pull_until(c: &pyxee_audio::pipeline::Consumer, want: usize) -> Vec<f32> {
    let mut out = Vec::new();
    let mut buf = vec![0f32; 960];
    let deadline = Instant::now() + Duration::from_secs(10);
    while out.len() < want && Instant::now() < deadline {
        let n = c.pull(&mut buf, 0);
        out.extend_from_slice(&buf[..n]);
        if n == 0 {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    out
}

#[test]
fn the_pipeline_drops_what_was_decoded_ahead_and_plays_from_the_target() {
    let (tx, rx) = channel();
    let p = Pipeline::new(RATE, tx, Volume::default());
    let c = p.consumer();
    p.push(track(1, 60_000, true));
    // Let it play a little (the producer also fills ~2 s ahead of the device).
    let first = pull_until(&c, 2 * 4800);
    assert_eq!(frame_of(first[0]), 0);
    std::thread::sleep(Duration::from_millis(100));

    assert!(p.seek_in_place(EntryId(1), 30_000));
    let after = pull_until(&c, 2 * 480);
    assert!(
        frame_of(after[0]) >= 30_000 * 48 && frame_of(after[0]) < 30_000 * 48 + 48_000,
        "the first sample after the seek comes from the target, not from the old decoded-ahead audio: {}",
        frame_of(after[0])
    );
    // The track is announced as audible again, counted from the seek.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut audible_again = false;
    while Instant::now() < deadline && !audible_again {
        let _ = pull_until(&c, 2 * 480);
        audible_again = rx
            .try_iter()
            .any(|e| matches!(e, PipeEvent::Heard(Heard::Audible(id)) if id == EntryId(1)));
    }
    assert!(
        audible_again,
        "the engine is told the track is audible at the new position"
    );
    let (id, ms) = p.heard_position().expect("a position after the seek");
    assert_eq!(id, EntryId(1));
    assert!(
        ms < 3_000,
        "heard time counts from the seek, not from the start: {ms} ms"
    );
}

#[test]
fn a_track_waiting_behind_the_one_that_is_sought_stays_queued() {
    let (tx, _rx) = channel();
    let p = Pipeline::new(RATE, tx, Volume::default());
    let c = p.consumer();
    p.push(track(1, 60_000, true));
    p.push(track(2, 1_000, true));
    let _ = pull_until(&c, 2 * 4800);
    assert!(p.seek_in_place(EntryId(1), 59_000));
    // 1 s of track 1 is left, then track 2 follows without a gap and without being asked for again.
    let rest = pull_until(&c, 2 * RATE as usize * 2 - 2 * 4800);
    let reached_second =
        rest.iter().any(|s| frame_of(*s) < 100 && *s != 0.0) || rest.len() > 2 * RATE as usize;
    assert!(
        reached_second,
        "the queued track played after the sought one ended"
    );
}

#[test]
fn seeking_something_that_is_not_playing_or_cannot_seek_returns_false_and_playback_goes_on() {
    let (tx, _rx) = channel();
    let p = Pipeline::new(RATE, tx, Volume::default());
    let c = p.consumer();
    p.push(track(1, 60_000, false));
    let _ = pull_until(&c, 2 * 4800);
    assert!(
        !p.seek_in_place(EntryId(1), 5_000),
        "this source cannot seek in place"
    );
    assert!(!p.seek_in_place(EntryId(7), 5_000), "no such track");
    let more = pull_until(&c, 2 * 480);
    assert!(more.len() >= 2 * 480, "the track still plays");
}
