//! Joins tracks sample-accurately. The next track is opened and pre-decoded while the
//! current one plays, so the boundary is just "keep copying from the next buffer".

use crate::decode::SampleSource;
use crate::resample::StreamResampler;
use crate::{AudioError, CHANNELS};
use std::collections::VecDeque;

/// Pre-decode this much of a track before it is queued (frames at output rate / 4 ≈ 250 ms).
const PREFILL_DIVISOR: usize = 4;

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
    pending: Vec<f32>,
    pos: usize,
    src_done: bool,
    emitted: u64,
    scratch: Vec<f32>,
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
            pending: Vec::new(),
            pos: 0,
            src_done: false,
            emitted: 0,
            scratch: Vec::new(),
        };
        let target = out_rate as usize * CHANNELS / PREFILL_DIVISOR;
        while !t.src_done && t.pending.len() < target {
            t.decode_more()?;
        }
        Ok(t)
    }

    pub fn id(&self) -> u64 {
        self.id
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
}

impl GaplessSource {
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

    pub fn queued(&self) -> usize {
        self.queue.len() + usize::from(self.current.is_some())
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Fills `out` with interleaved stereo, crossing track boundaries without a gap.
    /// Returns the number of samples written; fewer than `out.len()` means nothing is queued.
    pub fn fill(&mut self, out: &mut [f32]) -> usize {
        let mut n = 0;
        while n < out.len() {
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
            let Some(track) = self.current.as_mut() else {
                break;
            };
            match track.copy(&mut out[n..]) {
                Ok(copied) => {
                    n += copied;
                    self.total_frames += (copied / CHANNELS) as u64;
                    if n < out.len() {
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
}
