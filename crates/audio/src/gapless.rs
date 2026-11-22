//! Joins tracks sample-accurately. The next track is opened and pre-decoded while the
//! current one plays, so the boundary is just "keep copying from the next buffer".
//!
//! With a crossfade the boundary is a blend instead: during the last seconds of a song the queued one starts under it
//! and the two are mixed with an equal-power curve. That needs the exact end of the song, and a stream does not say
//! where it ends until the decoder has got there, so a track that knows its length (see
//! `PreparedTrack::set_duration_hint`) is decoded a little ahead of the listener near its end. A track whose length is
//! unknown, or a next song that is not queued yet, simply joins gaplessly as before.

use crate::decode::SampleSource;
use crate::resample::StreamResampler;
use crate::{AudioError, CHANNELS};
use std::collections::VecDeque;

/// Pre-decode this much of a track before it is queued (frames at output rate / 4 ≈ 250 ms).
const PREFILL_DIVISOR: usize = 4;
/// A song's stated length is only an estimate. The decoder starts reading ahead this many seconds before the earliest
/// point where a crossfade could begin, so a length that is a few seconds too long still finds the real end in time.
const WATCH_MARGIN_SECS: u64 = 8;
/// A crossfade never takes more than this share (1/3) of a song, so a short song keeps most of its own length (and a
/// song that ends in the middle of the fade is never mistaken for one that was cut off).
const MAX_FADE_DIVISOR: usize = 3;
/// A fade shorter than this (frames at output rate / 20 ≈ 50 ms) is not worth doing: the songs join gaplessly.
const MIN_FADE_DIVISOR: u64 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// `at_frame` is the cumulative output frame where the track begins.
    Started { id: u64, at_frame: u64 },
    /// `frames_emitted` counts frames handed to the output, not wall-clock time or reported duration.
    /// `at_frame` is the cumulative output frame where the track ends (exclusive).
    Finished {
        id: u64,
        frames_emitted: u64,
        at_frame: u64,
    },
    Failed {
        id: u64,
        kind: FailKind,
        message: String,
        /// Frames of this track that were produced before it failed.
        frames_emitted: u64,
        at_frame: u64,
    },
}

/// Why a track stopped producing audio, in terms a caller can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailKind {
    /// The stream URL was rejected (HTTP 401/403/410): re-resolve and resume.
    Expired(u16),
    Network,
    /// The stream server answered HTTP 429: transient, like `Network`.
    RateLimited,
    Decode,
    /// The track decoded to nothing.
    Empty,
}

impl From<&AudioError> for FailKind {
    fn from(e: &AudioError) -> Self {
        match e {
            AudioError::UrlExpired(c) => FailKind::Expired(*c),
            AudioError::Network(_) => FailKind::Network,
            AudioError::RateLimited => FailKind::RateLimited,
            _ => FailKind::Decode,
        }
    }
}

/// A track that is opened, resampling-ready and partly decoded.
pub struct PreparedTrack {
    id: u64,
    src: Box<dyn SampleSource>,
    rs: StreamResampler,
    out_rate: u32,
    pending: Vec<f32>,
    pos: usize,
    src_done: bool,
    emitted: u64,
    scratch: Vec<f32>,
    /// How long the song is, if the caller knows (milliseconds), and where in it this track was opened.
    duration_ms: Option<u64>,
    start_ms: u64,
}

fn append_stereo(input: &[f32], channels: usize, out: &mut Vec<f32>) {
    match channels {
        1 => out.extend(input.iter().flat_map(|&s| [s, s])),
        2 => out.extend_from_slice(input),
        n => {
            for f in input.chunks_exact(n) {
                out.extend_from_slice(&f[..2]);
            }
        }
    }
}

impl PreparedTrack {
    /// Does the slow work (resampler setup and first decode) up front.
    pub fn new(id: u64, src: Box<dyn SampleSource>, out_rate: u32) -> Result<Self, AudioError> {
        let rs = StreamResampler::new(src.sample_rate(), out_rate)?;
        let mut t = Self {
            id,
            src,
            rs,
            out_rate,
            pending: Vec::new(),
            pos: 0,
            src_done: false,
            emitted: 0,
            scratch: Vec::new(),
            duration_ms: None,
            start_ms: 0,
        };
        let target = out_rate as usize * CHANNELS / PREFILL_DIVISOR;
        while !t.src_done && t.pending.len() < target {
            t.decode_more()?;
        }
        Ok(t)
    }

    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Tells the track how long its song is and where in it this track was opened (`start_ms`, 0 for a track that
    /// starts at the beginning). A crossfade needs this to know when the end is near; without it the track is only
    /// ever joined gaplessly. A length of 0 counts as unknown.
    pub fn set_duration_hint(&mut self, duration_ms: Option<u64>, start_ms: u64) {
        self.duration_ms = duration_ms.filter(|d| *d > 0);
        self.start_ms = start_ms;
    }

    /// How many frames this track is expected to produce in all, from the length it was given.
    fn expected_total_frames(&self) -> Option<u64> {
        let d = self.duration_ms?;
        Some(d.saturating_sub(self.start_ms) * u64::from(self.out_rate) / 1000)
    }

    /// Frames that are decoded and not played yet. Only the whole rest of the track once `src_done` is set.
    fn ready_frames(&self) -> usize {
        (self.pending.len() - self.pos) / CHANNELS
    }

    /// Decodes until more than `frames` are waiting, or the source ends (so with `src_done` still unset, there is
    /// more than `frames` left of the track). The listener's position does not move.
    fn decode_ahead(&mut self, frames: usize) -> Result<(), AudioError> {
        while !self.src_done && self.ready_frames() <= frames {
            // Played audio is dropped from the front once it is half of what is held, so this does not move the
            // whole buffer on every read.
            if self.pos > 0 && self.pos * 2 >= self.pending.len() {
                self.pending.drain(..self.pos);
                self.pos = 0;
            }
            self.decode_more()?;
        }
        Ok(())
    }

    /// Jumps to `ms` in the same open source: what was decoded ahead is dropped and the resampler starts over (a
    /// seek is a discontinuity anyway). Counting of `emitted` starts again, as it does for a track opened at an offset.
    /// On an error the track is in an unknown state and must be thrown away (the caller re-opens it).
    fn seek(&mut self, ms: u64) -> Result<(), AudioError> {
        self.src.seek_ms(ms)?;
        self.rs = StreamResampler::new(self.src.sample_rate(), self.out_rate)?;
        self.pending.clear();
        self.pos = 0;
        self.src_done = false;
        self.emitted = 0;
        self.start_ms = ms;
        Ok(())
    }

    fn decode_more(&mut self) -> Result<(), AudioError> {
        self.scratch.clear();
        if self.src.read(&mut self.scratch)? {
            let mut stereo = Vec::with_capacity(self.scratch.len());
            append_stereo(&self.scratch, self.src.channels(), &mut stereo);
            self.rs.process(&stereo, &mut self.pending)?;
        } else {
            self.src_done = true;
            self.rs.finish(&mut self.pending)?;
        }
        Ok(())
    }

    /// Copies up to `out.len()` samples; returns fewer only when the track has ended.
    fn copy(&mut self, out: &mut [f32]) -> Result<usize, AudioError> {
        let mut n = 0;
        while n < out.len() {
            if self.pos >= self.pending.len() {
                if self.src_done {
                    break;
                }
                self.pending.clear();
                self.pos = 0;
                self.decode_more()?;
                continue;
            }
            let take = (out.len() - n).min(self.pending.len() - self.pos);
            out[n..n + take].copy_from_slice(&self.pending[self.pos..self.pos + take]);
            self.pos += take;
            n += take;
        }
        self.emitted += (n / CHANNELS) as u64;
        Ok(n)
    }
}

#[derive(Default)]
pub struct GaplessSource {
    current: Option<PreparedTrack>,
    queue: VecDeque<PreparedTrack>,
    events: Vec<Event>,
    /// Cumulative frames produced so far; event positions are measured on this timeline.
    total_frames: u64,
    /// A crossfade under way: `current` is the song that is fading out and this is the one fading in.
    mix: Option<Mix>,
    /// Where the incoming song's share of a mixed block is decoded to (kept to avoid allocating per block).
    mix_scratch: Vec<f32>,
}

/// The song fading in during a crossfade.
struct Mix {
    incoming: PreparedTrack,
    /// Length of the fade and how much of it is done (frames).
    total: usize,
    done: usize,
    /// The incoming song ended (or broke) before the fade did; reported once the fade is over.
    end: Option<MixEnd>,
}

enum MixEnd {
    Finished,
    Failed(AudioError),
}

/// What to do with the song that is playing next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plan {
    /// Copy at most this many samples from it.
    Copy(usize),
    /// Start fading into the queued song; the fade is this many frames long.
    Mix(usize),
}

/// The gains for the outgoing and the incoming song at `t` (0 = the fade begins, 1 = it is over). Equal power:
/// the two squares always add up to 1, so the loudness stays level through the fade.
#[must_use]
pub fn fade_gains(t: f32) -> (f32, f32) {
    let angle = t.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (angle.cos(), angle.sin())
}

/// Decides when the fade begins. `fade` is the length wanted (frames), `min_fade` the shortest one worth doing,
/// `emitted` how much of the song has been played and `remaining` how much is left; the song's end is known (the
/// decoder has reached it). The fade is shortened to a third of a short song, and begins when exactly that much of
/// the song is left.
fn plan(fade: usize, min_fade: usize, emitted: usize, remaining: usize) -> Plan {
    let length = emitted + remaining;
    let fade = fade.min(length / MAX_FADE_DIVISOR);
    if fade < min_fade.max(1) || remaining == 0 {
        return Plan::Copy(usize::MAX);
    }
    if remaining > fade {
        Plan::Copy((remaining - fade) * CHANNELS)
    } else if remaining >= min_fade.max(1) {
        Plan::Mix(remaining)
    } else {
        Plan::Copy(usize::MAX) // too little left to fade: it joins gaplessly
    }
}

impl GaplessSource {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, track: PreparedTrack) {
        self.queue.push_back(track);
    }

    /// Drops tracks waiting behind the current one (a cancelled prefetch) without touching what is playing.
    pub fn clear_queued(&mut self) {
        self.queue.clear();
    }

    /// Seeks the track that is playing now (`id`) inside its open source, keeping whatever is queued behind it.
    /// The output timeline starts again at 0 and the track is announced as started there, which is what a track
    /// opened at the offset would do. Errors when `id` is not the track playing (it may be queued, or over) or the
    /// source cannot seek in place; the caller then re-opens the track.
    pub fn seek_current(&mut self, id: u64, ms: u64) -> Result<(), AudioError> {
        if let Some(mix) = self.mix.take_if(|m| m.incoming.id == id) {
            // The listener is in the song that is fading in and seeks in it: the one fading out is dropped.
            self.current = Some(mix.incoming);
        } else if self.mix.is_some() {
            return Err(AudioError::Unsupported(
                "a crossfade is under way; the caller re-opens the track".into(),
            ));
        }
        let Some(track) = self.current.as_mut().filter(|t| t.id == id) else {
            return Err(AudioError::Unsupported(
                "that track is not the one playing".into(),
            ));
        };
        track.seek(ms)?;
        self.total_frames = 0;
        self.events.push(Event::Started { id, at_frame: 0 });
        Ok(())
    }

    /// Makes the queued track `id` the one that plays now: the track that is playing and any track queued before `id`
    /// are dropped, and `id` is announced as started at the start of a new timeline, as a track opened by itself would
    /// be. This is a skip to a song that was prefetched: nothing is opened or decoded again. `false`, with nothing
    /// changed, when `id` is not queued (it may be playing already, or never have been prepared).
    pub fn skip_to_queued(&mut self, id: u64) -> bool {
        let Some(at) = self.queue.iter().position(|t| t.id == id) else {
            return false;
        };
        self.queue.drain(..at);
        self.current = None;
        self.mix = None;
        self.total_frames = 0;
        true
    }

    #[must_use]
    pub fn queued(&self) -> usize {
        self.queue.len() + usize::from(self.current.is_some()) + usize::from(self.mix.is_some())
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Fills `out` with interleaved stereo, crossing track boundaries without a gap.
    /// Returns the number of samples written; fewer than `out.len()` means nothing is queued.
    pub fn fill(&mut self, out: &mut [f32]) -> usize {
        self.fill_with(out, 0)
    }

    /// Like `fill`, with a crossfade of `fade_ms` milliseconds between songs (0 = none, a gapless join).
    pub fn fill_with(&mut self, out: &mut [f32], fade_ms: u32) -> usize {
        let mut n = 0;
        while n < out.len() {
            if self.mix.is_some() {
                let mixed = self.fill_mix(&mut out[n..]);
                if mixed == 0 {
                    break;
                }
                n += mixed;
                continue;
            }
            if self.current.is_none() {
                match self.queue.pop_front() {
                    Some(t) => {
                        self.events.push(Event::Started {
                            id: t.id,
                            at_frame: self.total_frames,
                        });
                        self.current = Some(t);
                    }
                    None => break,
                }
            }
            let limit = match self.plan_next(fade_ms) {
                Plan::Mix(frames) => {
                    self.start_mix(frames);
                    continue;
                }
                Plan::Copy(max) => max.min(out.len() - n),
            };
            let Some(track) = self.current.as_mut() else {
                break;
            };
            match track.copy(&mut out[n..n + limit]) {
                Ok(copied) => {
                    n += copied;
                    self.total_frames += (copied / CHANNELS) as u64;
                    if copied < limit {
                        let at_frame = self.total_frames;
                        self.events.push(if track.emitted == 0 {
                            Event::Failed {
                                id: track.id,
                                kind: FailKind::Empty,
                                message: "track produced no audio".into(),
                                frames_emitted: 0,
                                at_frame,
                            }
                        } else {
                            Event::Finished {
                                id: track.id,
                                frames_emitted: track.emitted,
                                at_frame,
                            }
                        });
                        self.current = None;
                    }
                }
                Err(e) => {
                    self.events.push(Event::Failed {
                        id: track.id,
                        kind: FailKind::from(&e),
                        message: e.to_string(),
                        frames_emitted: track.emitted,
                        at_frame: self.total_frames,
                    });
                    self.current = None;
                }
            }
        }
        n
    }

    /// Whether the song that plays should be copied on as it is or start fading into the queued one. Near the end of a
    /// song of known length this also reads the decoder ahead, so that its real end is known when the fade has to begin.
    fn plan_next(&mut self, fade_ms: u32) -> Plan {
        const NONE: Plan = Plan::Copy(usize::MAX);
        if fade_ms == 0 || self.queue.is_empty() {
            return NONE;
        }
        let Some(track) = self.current.as_mut() else {
            return NONE;
        };
        let rate = u64::from(track.out_rate);
        let fade = (u64::from(fade_ms) * rate / 1000) as usize;
        let Some(total) = track.expected_total_frames() else {
            return NONE;
        };
        let watch_from = total.saturating_sub(fade as u64 + WATCH_MARGIN_SECS * rate);
        if track.emitted < watch_from {
            return NONE;
        }
        // A read that fails here fails again when the song is copied, which reports it; nothing to do about it now.
        if track.decode_ahead(fade).is_err() {
            return NONE;
        }
        if !track.src_done {
            // More than `fade` is left. Copy no further than the point where exactly `fade` is, and look again there:
            // that is where the fade begins if the decoder finds the end by then.
            return Plan::Copy((track.ready_frames() - fade) * CHANNELS);
        }
        plan(
            fade,
            (rate / MIN_FADE_DIVISOR) as usize,
            track.emitted as usize,
            track.ready_frames(),
        )
    }

    /// The queued song starts under the one that plays. The song that plays is announced as over here, and the next
    /// one as started: the window moves on to it as the fade begins, as it does in other players.
    fn start_mix(&mut self, frames: usize) {
        let Some(next) = self.queue.pop_front() else {
            return;
        };
        if let Some(out) = self.current.as_ref() {
            self.events.push(Event::Finished {
                id: out.id,
                frames_emitted: out.emitted,
                at_frame: self.total_frames,
            });
        }
        self.events.push(Event::Started {
            id: next.id,
            at_frame: self.total_frames,
        });
        self.mix = Some(Mix {
            incoming: next,
            total: frames,
            done: 0,
            end: None,
        });
    }

    /// Writes the next block of a crossfade into `out`; returns the samples written (0: no room for a frame).
    fn fill_mix(&mut self, out: &mut [f32]) -> usize {
        let Some(mut mix) = self.mix.take() else {
            return 0;
        };
        let frames = (out.len() / CHANNELS).min(mix.total - mix.done);
        if frames == 0 {
            self.mix = Some(mix);
            return 0;
        }
        let samples = frames * CHANNELS;
        let out = &mut out[..samples];
        // The song fading out. Its end was counted when the fade was planned, so it has what the fade needs; if it
        // fails anyway, it is silent from there and the fade carries on.
        let got = match self.current.as_mut().map(|a| a.copy(out)) {
            Some(Ok(copied)) => copied,
            Some(Err(e)) => {
                log::warn!("the song fading out failed during the crossfade: {e}");
                self.current = None;
                0
            }
            None => 0,
        };
        out[got..].fill(0.0);
        // The song fading in.
        let mut incoming = std::mem::take(&mut self.mix_scratch);
        incoming.clear();
        incoming.resize(samples, 0.0);
        let got = if mix.end.is_some() {
            0
        } else {
            match mix.incoming.copy(&mut incoming) {
                Ok(copied) => {
                    if copied < samples {
                        mix.end = Some(MixEnd::Finished);
                    }
                    copied
                }
                Err(e) => {
                    mix.end = Some(MixEnd::Failed(e));
                    0
                }
            }
        };
        incoming[got..].fill(0.0);
        for (i, (o, b)) in out
            .chunks_exact_mut(CHANNELS)
            .zip(incoming.chunks_exact(CHANNELS))
            .enumerate()
        {
            let t = ((mix.done + i) as f32 + 0.5) / mix.total as f32;
            let (fade_out, fade_in) = fade_gains(t);
            for (o, b) in o.iter_mut().zip(b) {
                *o = *o * fade_out + *b * fade_in;
            }
        }
        self.mix_scratch = incoming;
        self.total_frames += frames as u64;
        mix.done += frames;
        if mix.done >= mix.total {
            self.finish_mix(mix);
        } else {
            self.mix = Some(mix);
        }
        samples
    }

    /// The fade is over: the incoming song plays on alone (unless it ended during the fade, which is reported now).
    fn finish_mix(&mut self, mix: Mix) {
        self.current = None;
        let Mix { incoming, end, .. } = mix;
        let at_frame = self.total_frames;
        match end {
            None => self.current = Some(incoming),
            Some(MixEnd::Finished) if incoming.emitted > 0 => self.events.push(Event::Finished {
                id: incoming.id,
                frames_emitted: incoming.emitted,
                at_frame,
            }),
            Some(MixEnd::Finished) => self.events.push(Event::Failed {
                id: incoming.id,
                kind: FailKind::Empty,
                message: "track produced no audio".into(),
                frames_emitted: 0,
                at_frame,
            }),
            Some(MixEnd::Failed(e)) => self.events.push(Event::Failed {
                id: incoming.id,
                kind: FailKind::from(&e),
                message: e.to_string(),
                frames_emitted: incoming.emitted,
                at_frame,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fade_keeps_the_power_level_all_the_way() {
        for i in 0..=100 {
            let (out, inc) = fade_gains(i as f32 / 100.0);
            assert!((out * out + inc * inc - 1.0).abs() < 1e-5, "step {i}");
        }
        assert_eq!(fade_gains(0.0), (1.0, 0.0));
        let (out, inc) = fade_gains(1.0);
        assert!(out.abs() < 1e-6 && (inc - 1.0).abs() < 1e-6);
        assert_eq!(fade_gains(-3.0), fade_gains(0.0), "out of range is clamped");
        assert_eq!(fade_gains(9.0), fade_gains(1.0));
    }

    #[test]
    fn the_fade_begins_when_exactly_that_much_of_the_song_is_left() {
        // 10 s of 48 kHz: 4 s into it, 6 s are left; a 2 s fade is 96 000 frames.
        let (fade, min) = (96_000, 2_400);
        assert_eq!(
            plan(fade, min, 192_000, 288_000),
            Plan::Copy((288_000 - 96_000) * CHANNELS)
        );
        assert_eq!(plan(fade, min, 384_000, 96_000), Plan::Mix(96_000));
    }

    #[test]
    fn a_short_song_gets_a_fade_of_a_third_of_itself_at_most() {
        // 3 s long (144 000 frames), 12 s asked for: the fade is 1 s.
        assert_eq!(
            plan(576_000, 2_400, 0, 144_000),
            Plan::Copy((144_000 - 48_000) * CHANNELS)
        );
        assert_eq!(plan(576_000, 2_400, 96_000, 48_000), Plan::Mix(48_000));
    }

    #[test]
    fn a_fade_too_short_to_matter_is_a_gapless_join() {
        assert_eq!(plan(0, 2_400, 100, 100), Plan::Copy(usize::MAX));
        assert_eq!(
            plan(96_000, 2_400, 0, 3_000),
            Plan::Copy(usize::MAX),
            "a third of 3 000 frames is 1 000"
        );
        assert_eq!(
            plan(96_000, 2_400, 480_000, 0),
            Plan::Copy(usize::MAX),
            "nothing left"
        );
        // The decoder found the end late and little is left: no point in a fade of a few frames.
        assert_eq!(plan(96_000, 2_400, 480_000, 100), Plan::Copy(usize::MAX));
    }
}
