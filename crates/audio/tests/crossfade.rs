//! Skipping to a track that was prepared and queued: the track that was playing is dropped, the queued one starts at
//! once from its beginning, and a track that is not queued is refused without changing anything.

use pyxee_audio::AudioError;
use pyxee_audio::decode::SampleSource;
use pyxee_audio::gapless::{Event, GaplessSource, PreparedTrack};

const RATE: u32 = 48_000;
const LEVEL: f32 = 0.25;

/// Stereo at the output rate (nothing is resampled): every sample is `LEVEL`, so a mix of two of them shows how the
/// fade treats the level.
struct Tone {
    frame: u64,
    total: u64,
}

impl Tone {
    fn new(total_ms: u64) -> Self {
        Self {
            frame: 0,
            total: total_ms * u64::from(RATE) / 1000,
        }
    }
}

impl SampleSource for Tone {
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
        for _ in 0..n {
            out.extend([LEVEL, LEVEL]);
        }
        self.frame += n;
        Ok(true)
    }
}

fn secs(s: u64) -> u64 {
    s * u64::from(RATE)
}

/// A track of `ms` milliseconds that was told its song is `hint_ms` long (`None`: unknown).
fn track(id: u64, ms: u64, hint_ms: Option<u64>) -> PreparedTrack {
    let mut t = PreparedTrack::new(id, Box::new(Tone::new(ms)), RATE).unwrap();
    t.set_duration_hint(hint_ms, 0);
    t
}

/// Everything `fill_with` produces until nothing is left, in blocks of an odd size so that block edges do not line up
/// with the fade, plus the events.
fn render(g: &mut GaplessSource, fade_ms: u32) -> (Vec<f32>, Vec<Event>) {
    let mut all = Vec::new();
    let mut buf = vec![0f32; 2 * 1531];
    loop {
        let n = g.fill_with(&mut buf, fade_ms);
        all.extend_from_slice(&buf[..n]);
        if n < buf.len() {
            break;
        }
    }
    (all, g.drain_events())
}

fn two(first: (u64, Option<u64>), second: (u64, Option<u64>)) -> GaplessSource {
    let mut g = GaplessSource::new();
    g.push(track(1, first.0, first.1));
    g.push(track(2, second.0, second.1));
    g
}

#[test]
fn the_songs_overlap_by_the_fade_and_the_window_moves_on_when_it_begins() {
    let mut g = two((10_000, Some(10_000)), (10_000, Some(10_000)));
    let (out, events) = render(&mut g, 2_000);
    assert_eq!(
        out.len() as u64 / 2,
        secs(18),
        "10 s + 10 s - 2 s of overlap"
    );
    assert_eq!(
        events,
        vec![
            Event::Started { id: 1, at_frame: 0 },
            Event::Finished {
                id: 1,
                frames_emitted: secs(8),
                at_frame: secs(8)
            },
            Event::Started {
                id: 2,
                at_frame: secs(8)
            },
            Event::Finished {
                id: 2,
                frames_emitted: secs(10),
                at_frame: secs(18)
            },
        ]
    );
}

#[test]
fn the_level_never_dips_through_the_fade() {
    let mut g = two((10_000, Some(10_000)), (10_000, Some(10_000)));
    let (out, _) = render(&mut g, 2_000);
    let frame = |i: u64| out[2 * i as usize];
    for i in (0..secs(8)).step_by(97) {
        assert!(
            (frame(i) - LEVEL).abs() < 1e-6,
            "before the fade, frame {i}"
        );
    }
    for i in (secs(8)..secs(10)).step_by(7) {
        // Equal power: cos + sin is never below 1 and at most sqrt(2).
        assert!(frame(i) >= LEVEL - 1e-6, "dip at frame {i}: {}", frame(i));
        assert!(
            frame(i) <= LEVEL * 1.4143,
            "peak at frame {i}: {}",
            frame(i)
        );
    }
    for i in (secs(10)..secs(18)).step_by(97) {
        assert!((frame(i) - LEVEL).abs() < 1e-6, "after the fade, frame {i}");
    }
    // Both channels carry the same thing.
    assert!(
        out.as_chunks::<2>()
            .0
            .iter()
            .all(|f| (f[0] - f[1]).abs() < 1e-6)
    );
}

#[test]
fn a_song_of_unknown_length_joins_the_next_without_a_gap_and_without_a_fade() {
    let mut g = two((10_000, None), (10_000, Some(10_000)));
    let (out, events) = render(&mut g, 2_000);
    assert_eq!(out.len() as u64 / 2, secs(20));
    assert!(
        out.iter().all(|s| (s - LEVEL).abs() < 1e-6),
        "no mixing happened"
    );
    assert!(events.contains(&Event::Started {
        id: 2,
        at_frame: secs(10)
    }));
}

#[test]
fn a_fade_of_zero_is_the_gapless_join_it_always_was() {
    let mut g = two((10_000, Some(10_000)), (10_000, Some(10_000)));
    let (out, events) = render(&mut g, 0);
    assert_eq!(out.len() as u64 / 2, secs(20));
    assert!(events.contains(&Event::Finished {
        id: 1,
        frames_emitted: secs(10),
        at_frame: secs(10)
    }));
}

#[test]
fn without_a_next_song_the_last_one_plays_to_its_end() {
    let mut g = GaplessSource::new();
    g.push(track(1, 10_000, Some(10_000)));
    let (out, events) = render(&mut g, 4_000);
    assert_eq!(out.len() as u64 / 2, secs(10));
    assert_eq!(
        events.last(),
        Some(&Event::Finished {
            id: 1,
            frames_emitted: secs(10),
            at_frame: secs(10)
        })
    );
}

#[test]
fn a_short_song_is_faded_over_a_third_of_itself_at_most() {
    let mut g = two((3_000, Some(3_000)), (10_000, Some(10_000)));
    let (out, events) = render(&mut g, 12_000);
    // The fade is 1 s (a third of 3 s), so the second song starts 2 s in.
    assert_eq!(out.len() as u64 / 2, secs(3 - 1 + 10));
    assert!(events.contains(&Event::Started {
        id: 2,
        at_frame: secs(2)
    }));
}

#[test]
fn a_stated_length_that_is_a_few_seconds_too_long_still_finds_the_real_end() {
    // The song says 15 s but is 10 s: the decoder was already reading ahead, so the fade still ends with the song.
    let mut g = two((10_000, Some(15_000)), (10_000, Some(10_000)));
    let (out, events) = render(&mut g, 2_000);
    assert_eq!(out.len() as u64 / 2, secs(18));
    assert!(events.contains(&Event::Started {
        id: 2,
        at_frame: secs(8)
    }));
}

#[test]
fn a_stated_length_that_is_far_too_long_falls_back_to_the_gapless_join() {
    let mut g = two((10_000, Some(60_000)), (10_000, Some(10_000)));
    let (out, events) = render(&mut g, 2_000);
    assert_eq!(out.len() as u64 / 2, secs(20));
    assert!(events.contains(&Event::Started {
        id: 2,
        at_frame: secs(10)
    }));
}

/// A crossfade of two 10 s songs, read until half a second into the fade.
fn into_the_fade() -> GaplessSource {
    let mut g = two((10_000, Some(10_000)), (10_000, Some(10_000)));
    let mut buf = vec![0f32; 2 * 4800];
    let mut produced = 0;
    while produced < secs(8) + secs(1) / 2 {
        produced += g.fill_with(&mut buf, 2_000) as u64 / 2;
    }
    let _ = g.drain_events();
    g
}

#[test]
fn a_seek_aimed_at_the_song_fading_out_is_refused_and_leaves_the_fade_alone() {
    let mut g = into_the_fade();
    assert!(g.seek_current(1, 0).is_err());
    assert_eq!(g.queued(), 2, "both songs are still there");
}

#[test]
fn a_seek_aimed_at_the_song_fading_in_ends_the_fade() {
    let mut g = into_the_fade();
    // These tones cannot seek in place, so the seek itself fails; what counts is that the song fading out is gone and
    // the other one is the song that plays (the caller re-opens it, as it does for any track that cannot seek).
    assert!(g.seek_current(2, 0).is_err());
    assert_eq!(g.queued(), 1);
}

#[test]
fn skipping_to_a_queued_song_ends_the_fade() {
    let mut g = into_the_fade();
    g.push(track(3, 10_000, Some(10_000)));
    assert!(g.skip_to_queued(3));
    assert_eq!(g.queued(), 1, "only the song skipped to is left");
}
