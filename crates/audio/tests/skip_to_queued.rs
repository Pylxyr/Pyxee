//! Skipping to a track that was prepared and queued: the track that was playing is dropped, the queued one starts at
//! once from its beginning, and a track that is not queued is refused without changing anything.

// Pedantic lints this project does not follow, allowed once per crate root. (A workspace-level allow loses to the
// `-D clippy::pedantic` that is passed on the command line, so it has to be an attribute.) Each is a trade-off for
// application code: `# Errors` / `# Panics` sections and `#[must_use]` on internal items, long match-heavy functions,
// flag structs, short names in numeric code, and the casts between sample counts, milliseconds and pixels (their
// ranges are bounded by what the device and the files can hold) are not worth the noise.
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::too_many_lines,
    clippy::struct_excessive_bools,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

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
}

impl Ramp {
    fn new(total_ms: u64) -> Self {
        Self {
            frame: 0,
            total: total_ms * u64::from(RATE) / 1000,
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
    fn seek_ms(&mut self, _ms: u64) -> Result<(), AudioError> {
        Err(AudioError::Unsupported("no seeking here".into()))
    }
}

fn frame_of(sample: f32) -> u64 {
    (sample * 1.0e6).round() as u64
}

fn track(id: u64, ms: u64) -> PreparedTrack {
    PreparedTrack::new(id, Box::new(Ramp::new(ms)), RATE).unwrap()
}

#[test]
fn the_queued_track_starts_at_once_from_its_beginning_and_the_playing_one_is_dropped() {
    let mut g = GaplessSource::new();
    g.push(track(1, 10_000));
    g.push(track(2, 10_000));
    let mut buf = vec![0f32; 2 * 4800];
    assert_eq!(g.fill(&mut buf), buf.len());
    let _ = g.drain_events(); // Started{1, 0}

    assert!(g.skip_to_queued(2));
    assert_eq!(g.fill(&mut buf), buf.len());
    assert_eq!(frame_of(buf[0]), 0, "track 2 plays from its start");
    assert_eq!(
        g.drain_events(),
        vec![Event::Started { id: 2, at_frame: 0 }],
        "the new timeline starts with the new track, and track 1 never finishes"
    );
}

#[test]
fn tracks_queued_before_the_wanted_one_are_dropped_and_those_after_it_stay() {
    let mut g = GaplessSource::new();
    g.push(track(1, 10_000));
    g.push(track(2, 10_000));
    g.push(track(3, 10_000));
    g.push(track(4, 10_000));
    let mut buf = vec![0f32; 2 * 480];
    g.fill(&mut buf);
    let _ = g.drain_events();
    assert!(g.skip_to_queued(3));
    assert_eq!(g.queued(), 2, "track 3 is next to play, track 4 follows it");
    g.fill(&mut buf);
    assert_eq!(
        g.drain_events(),
        vec![Event::Started { id: 3, at_frame: 0 }]
    );
}

#[test]
fn a_track_that_is_not_queued_is_refused_and_nothing_changes() {
    let mut g = GaplessSource::new();
    g.push(track(1, 10_000));
    g.push(track(2, 10_000));
    let mut buf = vec![0f32; 2 * 480];
    g.fill(&mut buf);
    let _ = g.drain_events();
    // Track 1 is playing (not queued); 9 does not exist.
    assert!(!g.skip_to_queued(1));
    assert!(!g.skip_to_queued(9));
    g.fill(&mut buf);
    assert_eq!(frame_of(buf[0]), 480, "playback went on from where it was");
    assert!(g.drain_events().is_empty());
    assert_eq!(g.queued(), 2);
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
fn in_the_pipeline_the_skip_drops_what_was_decoded_ahead_and_announces_the_new_track() {
    let (tx, rx) = channel();
    let p = Pipeline::new(RATE, tx, Volume::new(100));
    let c = p.consumer();
    p.push(track(1, 20_000));
    p.push(track(2, 20_000));
    let heard = pull_until(&c, 2 * 4800);
    assert!(heard.len() >= 2 * 4800);

    assert!(p.skip_to_queued(EntryId(2)));
    let after = pull_until(&c, 2 * 4800);
    assert!(after.len() >= 2 * 4800);
    assert_eq!(frame_of(after[0]), 0, "nothing of track 1 is left to hear");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut audible = Vec::new();
    while Instant::now() < deadline && !audible.contains(&EntryId(2)) {
        while let Ok(e) = rx.try_recv() {
            if let PipeEvent::Heard(Heard::Audible(id)) = e {
                audible.push(id);
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(audible, vec![EntryId(1), EntryId(2)]);
    assert!(
        !p.skip_to_queued(EntryId(7)),
        "a track that was never pushed is refused"
    );
}
