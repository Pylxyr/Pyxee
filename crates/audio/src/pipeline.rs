//! The playback pipeline: a producer thread decodes queued tracks (gaplessly) into a small buffer,
//! the output device drains it through a `Consumer`, and a `HeardTracker` turns "frames the device
//! consumed" into track-start/track-end events. Flushing is instant (skip, seek, stop).
//!
//! Lock order is always `source` then `inner`; the device callback only ever takes `inner`.

use crate::gapless::{Event, FailKind, GaplessSource, PreparedTrack};
use crate::CHANNELS;
use pyxee_core::{EntryId, Heard, HeardTracker};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Seconds of audio kept decoded ahead of the device.
const BUFFER_SECS: usize = 2;
const CHUNK_SAMPLES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipeEvent {
    /// A track became audible or finished being heard.
    Heard(Heard),
    /// A track stopped producing audio (decode error, expired URL, empty).
    Failed {
        id: EntryId,
        kind: FailKind,
        message: String,
        /// Frames of the track that played before it failed (where to resume from).
        frames_emitted: u64,
    },
}

struct Inner {
    buf: VecDeque<f32>,
    tracker: HeardTracker,
    consumed: u64,
    tx: Sender<PipeEvent>,
}

struct Shared {
    source: Mutex<GaplessSource>,
    inner: Mutex<Inner>,
    paused: AtomicBool,
    stop: AtomicBool,
    queued_tracks: AtomicUsize,
    underruns: AtomicU64,
    rate: u32,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Pipeline {
    shared: Arc<Shared>,
    producer: Option<JoinHandle<()>>,
}

/// Handed to the output device; `pull` is called from the audio callback.
#[derive(Clone)]
pub struct Consumer {
    shared: Arc<Shared>,
}

impl Pipeline {
    pub fn new(rate: u32, tx: Sender<PipeEvent>) -> Self {
        let shared = Arc::new(Shared {
            source: Mutex::new(GaplessSource::new()),
            inner: Mutex::new(Inner {
                buf: VecDeque::new(),
                tracker: HeardTracker::new(0),
                consumed: 0,
                tx,
            }),
            paused: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            queued_tracks: AtomicUsize::new(0),
            underruns: AtomicU64::new(0),
            rate,
        });
        let s = Arc::clone(&shared);
        let producer = thread::spawn(move || produce(&s));
        Self {
            shared,
            producer: Some(producer),
        }
    }

    pub fn consumer(&self) -> Consumer {
        Consumer {
            shared: Arc::clone(&self.shared),
        }
    }

    pub fn rate(&self) -> u32 {
        self.shared.rate
    }

    /// Queues a prepared track to follow whatever is already queued.
    pub fn push(&self, t: PreparedTrack) {
        lock(&self.shared.source).push(t);
    }

    /// Drops tracks queued behind the current one (e.g. a cancelled prefetch); playback continues.
    pub fn clear_queued(&self) {
        lock(&self.shared.source).clear_queued();
    }

    /// Drops everything queued and everything already buffered, immediately.
    pub fn flush(&self) {
        let mut src = lock(&self.shared.source);
        *src = GaplessSource::new();
        let mut inner = lock(&self.shared.inner);
        inner.buf.clear();
        inner.consumed = 0;
        inner.tracker.reset(0);
        self.shared.queued_tracks.store(0, Ordering::Relaxed);
    }

    /// While paused the device gets silence and nothing is consumed, so nothing "plays".
    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Relaxed);
    }

    /// The audible track and how many ms of it have actually been heard.
    pub fn heard_position(&self) -> Option<(EntryId, u64)> {
        let inner = lock(&self.shared.inner);
        inner
            .tracker
            .position()
            .map(|(id, frames)| (id, frames * 1000 / u64::from(self.shared.rate)))
    }

    pub fn underruns(&self) -> u64 {
        self.shared.underruns.load(Ordering::Relaxed)
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.producer.take() {
            let _ = h.join();
        }
    }
}

fn produce(s: &Shared) {
    let cap = s.rate as usize * CHANNELS * BUFFER_SECS;
    let mut tmp = vec![0f32; CHUNK_SAMPLES];
    while !s.stop.load(Ordering::Relaxed) {
        let free = cap.saturating_sub(lock(&s.inner).buf.len());
        if free < CHUNK_SAMPLES {
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        let mut src = lock(&s.source);
        let n = src.fill(&mut tmp);
        let events = src.drain_events();
        s.queued_tracks.store(src.queued(), Ordering::Relaxed);
        if n == 0 && events.is_empty() {
            drop(src);
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        let mut inner = lock(&s.inner);
        inner.buf.extend(&tmp[..n]);
        for e in events {
            match e {
                Event::Started { id, at_frame } => {
                    inner.tracker.track_started(EntryId(id), at_frame)
                }
                Event::Finished { id, at_frame, .. } => {
                    inner.tracker.track_ended(EntryId(id), at_frame)
                }
                Event::Failed {
                    id,
                    kind,
                    message,
                    frames_emitted,
                    at_frame,
                } => {
                    inner.tracker.track_failed(EntryId(id), at_frame);
                    let _ = inner.tx.send(PipeEvent::Failed {
                        id: EntryId(id),
                        kind,
                        message,
                        frames_emitted,
                    });
                }
            }
        }
    }
}

impl Consumer {
    /// Fills `out` (interleaved stereo) from the buffer, padding with silence on underrun.
    /// `latency_frames` is how long the device takes to actually sound what it was just given.
    /// Returns the number of real (non-padding) samples.
    pub fn pull(&self, out: &mut [f32], latency_frames: u64) -> usize {
        let s = &*self.shared;
        if s.paused.load(Ordering::Relaxed) {
            out.fill(0.0);
            return 0;
        }
        let mut inner = lock(&s.inner);
        let n = out.len().min(inner.buf.len());
        for (o, v) in out.iter_mut().zip(inner.buf.drain(..n)) {
            *o = v;
        }
        out[n..].fill(0.0);
        if n < out.len() && s.queued_tracks.load(Ordering::Relaxed) > 0 {
            s.underruns.fetch_add(1, Ordering::Relaxed);
        }
        inner.consumed += (n / CHANNELS) as u64;
        inner.tracker.set_latency(latency_frames);
        let consumed = inner.consumed;
        let heard = inner.tracker.advance(consumed);
        for h in heard {
            let _ = inner.tx.send(PipeEvent::Heard(h));
        }
        n
    }
}
