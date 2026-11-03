//! The playback pipeline: a producer thread decodes queued tracks (gaplessly) into a small buffer,
//! the output device drains it through a `Consumer`, and a `HeardTracker` turns "frames the device
//! consumed" into track-start/track-end events. Flushing is instant (skip, seek, stop).
//!
//! Lock order is always `source`, then `inner` or `pending`; the device callback only ever takes `inner`. The engine
//! thread never waits for the producer: `push` and `clear_queued` leave a note in `pending` for it to apply, and `flush`
//! cancels a read that is waiting for the network (see `Pipeline::cancel_token`).

use crate::gapless::{Event, FailKind, GaplessSource, PreparedTrack};
use crate::CHANNELS;
use pyxee_core::{EntryId, Heard, HeardTracker};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, TryLockError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Output volume, 0-100, shared between the UI side (which sets it) and the audio callback.
/// The slider is perceptual, so the gain follows a curve (percent squared): 50% is a quarter of the
/// amplitude (-12 dB), which sounds like "half as loud" far better than a straight line would.
#[derive(Clone)]
pub struct Volume {
    percent: Arc<AtomicU32>,
    /// A multiplier in thousandths (1000 = unchanged) that the sleep timer uses to fade out; it never
    /// touches the user's volume setting.
    fade_permille: Arc<AtomicU32>,
}

pub fn gain_for_percent(percent: u8) -> f32 {
    let x = f32::from(percent.min(100)) / 100.0;
    x * x
}

impl Volume {
    pub fn new(percent: u8) -> Self {
        Self {
            percent: Arc::new(AtomicU32::new(u32::from(percent.min(100)))),
            fade_permille: Arc::new(AtomicU32::new(1000)),
        }
    }
    pub fn set_percent(&self, percent: u8) {
        self.percent
            .store(u32::from(percent.min(100)), Ordering::Relaxed);
    }
    pub fn percent(&self) -> u8 {
        self.percent.load(Ordering::Relaxed) as u8
    }
    /// 1.0 = no fade, 0.0 = silent. Values outside 0..=1 are clamped.
    pub fn set_fade(&self, factor: f32) {
        let f = if factor.is_finite() {
            factor.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.fade_permille
            .store((f * 1000.0).round() as u32, Ordering::Relaxed);
    }
    pub fn fade(&self) -> f32 {
        self.fade_permille.load(Ordering::Relaxed) as f32 / 1000.0
    }
    pub fn gain(&self) -> f32 {
        gain_for_percent(self.percent()) * self.fade()
    }
}

impl Default for Volume {
    fn default() -> Self {
        Self::new(100)
    }
}

/// Seconds of audio kept decoded ahead of the device.
const BUFFER_SECS: usize = 2;
const CHUNK_SAMPLES: usize = 4096;
/// Linux: the longest the producer sleeps with a full buffer or with nothing queued. Every change that matters wakes it
/// at once; these only bound the harm if a wake-up were ever missed.
const FULL_LIMIT: Duration = Duration::from_millis(250);
const IDLE_LIMIT: Duration = Duration::from_millis(500);
/// Linux: with a full buffer the producer waits until this fraction of it (1/4: half a second of audio) is free,
/// then fills it up in one go, so playing costs a few wake-ups a second instead of one per chunk.
const REFILL_DIVISOR: usize = 4;

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
    /// The gain applied to the previous buffer; the next one ramps from here to the current target.
    gain: f32,
}

/// Something the engine thread wants done to the source, which only the producer thread may touch while it reads.
enum Change {
    Push(Box<PreparedTrack>),
    ClearQueued,
}

/// What the producer thread sleeps on. On Linux it blocks until something changes (a track was queued, the buffer
/// has room again, the pipeline is dropped), so an idle player wakes it a few times a second instead of every few
/// milliseconds; elsewhere it still sleeps the fixed time it is given.
struct Wake {
    epoch: Mutex<u64>,
    cv: Condvar,
    /// The producer is waiting for room in the buffer: the device side wakes it once there is enough.
    space_wanted: AtomicBool,
    /// How many times the producer's loop has run (an idle pipeline keeps this nearly still).
    runs: AtomicU64,
}

impl Wake {
    fn new() -> Self {
        Self {
            epoch: Mutex::new(0),
            cv: Condvar::new(),
            space_wanted: AtomicBool::new(false),
            runs: AtomicU64::new(0),
        }
    }

    /// Something the producer must look at changed.
    fn notify(&self) {
        *lock(&self.epoch) += 1;
        self.cv.notify_one();
    }

    /// Taken before the producer looks at its work: a `notify` after this makes the next `wait` return at once.
    fn epoch(&self) -> u64 {
        self.runs.fetch_add(1, Ordering::Relaxed);
        *lock(&self.epoch)
    }

    /// Waits for a `notify` since `seen`, but not longer than `limit`. Off Linux: sleeps `poll`.
    fn wait(&self, seen: u64, limit: Duration, poll: Duration) {
        if cfg!(target_os = "linux") {
            let g = lock(&self.epoch);
            if *g == seen {
                drop(self.cv.wait_timeout(g, limit));
            }
        } else {
            thread::sleep(poll);
        }
    }

    /// Like `wait`, for a full buffer: the device side calls `space_freed` once enough has been played.
    fn wait_for_space(&self, seen: u64, limit: Duration, poll: Duration) {
        self.space_wanted
            .store(cfg!(target_os = "linux"), Ordering::Relaxed);
        self.wait(seen, limit, poll);
    }

    /// The device took audio out of the buffer: wake a producer that waits for room, once there is enough of it.
    fn space_freed(&self, free: usize, enough: usize) {
        if free >= enough
            && self.space_wanted.load(Ordering::Relaxed)
            && self.space_wanted.swap(false, Ordering::Relaxed)
        {
            self.notify();
        }
    }
}

struct Shared {
    wake: Wake,
    source: Mutex<GaplessSource>,
    /// Changes waiting for the producer (taken only briefly; never held while reading).
    pending: Mutex<Vec<Change>>,
    inner: Mutex<Inner>,
    paused: AtomicBool,
    stop: AtomicBool,
    /// Set while a flush (or the drop) waits for the producer: reads that are waiting for the network give up.
    cancel: Arc<AtomicBool>,
    queued_tracks: AtomicUsize,
    underruns: AtomicU64,
    volume: Volume,
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
    pub fn new(rate: u32, tx: Sender<PipeEvent>, volume: Volume) -> Self {
        let shared = Arc::new(Shared {
            wake: Wake::new(),
            source: Mutex::new(GaplessSource::new()),
            pending: Mutex::new(Vec::new()),
            inner: Mutex::new(Inner {
                buf: VecDeque::new(),
                tracker: HeardTracker::new(0),
                consumed: 0,
                tx,
                gain: volume.gain(),
            }),
            paused: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            cancel: Arc::new(AtomicBool::new(false)),
            queued_tracks: AtomicUsize::new(0),
            underruns: AtomicU64::new(0),
            volume,
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

    /// Hand this to `HttpRangeSource::open_with_cancel`: while a flush is under way, a read of that source that is
    /// still waiting for data stops (it would otherwise hold the source lock for as long as the stream stalls).
    pub fn cancel_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shared.cancel)
    }

    /// Queues a prepared track to follow whatever is already queued. Does not wait for the producer, which may be
    /// stuck reading a stalled stream: the track joins the queue as soon as it is free.
    pub fn push(&self, t: PreparedTrack) {
        lock(&self.shared.pending).push(Change::Push(Box::new(t)));
        self.shared.wake.notify();
    }

    /// Drops tracks queued behind the current one (e.g. a cancelled prefetch); playback continues. Like `push`, it
    /// takes effect when the producer next looks, and never waits for it.
    pub fn clear_queued(&self) {
        lock(&self.shared.pending).push(Change::ClearQueued);
        self.shared.wake.notify();
    }

    /// Drops everything queued and everything already buffered, immediately.
    pub fn flush(&self) {
        // The producer holds the source lock while it reads, and a read can wait on a stalled stream for a
        // long time: tell it to give up before asking for the lock.
        self.shared.cancel.store(true, Ordering::SeqCst);
        let mut src = lock(&self.shared.source);
        *src = GaplessSource::new();
        lock(&self.shared.pending).clear();
        self.shared.cancel.store(false, Ordering::SeqCst);
        let mut inner = lock(&self.shared.inner);
        inner.buf.clear();
        inner.consumed = 0;
        inner.tracker.reset(0);
        self.shared.queued_tracks.store(0, Ordering::Relaxed);
        drop(inner);
        drop(src);
        self.shared.wake.notify();
    }

    /// How many times the producer thread's loop has run since the pipeline was made (a test hook: an idle pipeline
    /// must keep this nearly still).
    pub fn producer_runs(&self) -> u64 {
        self.shared.wake.runs.load(Ordering::Relaxed)
    }

    /// Seeks the track that is playing inside its open source, without re-opening it: the decoded-ahead audio is
    /// dropped, tracks queued behind it stay queued, and the track is announced as started again at the new position.
    /// `false` (nothing was changed that a flush does not repair): the track is not the one playing, its source cannot
    /// seek in place, or the producer is busy (a stalled stream), and the caller flushes and re-opens it instead.
    pub fn seek_in_place(&self, id: EntryId, ms: u64) -> bool {
        // Not `cancel`: unlike a flush this must not break the read that is under way. A producer that is waiting for
        // the network holds the lock for as long as that takes, so give up after a moment.
        let deadline = Instant::now() + Duration::from_millis(200);
        let mut src = loop {
            match self.shared.source.try_lock() {
                Ok(guard) => break guard,
                Err(TryLockError::Poisoned(e)) => break e.into_inner(),
                Err(TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return false;
                    }
                    thread::sleep(Duration::from_millis(2));
                }
            }
        };
        if let Err(e) = src.seek_current(id.0, ms) {
            log::debug!("seek in place not possible: {e}");
            return false;
        }
        // The producer holds the source lock while it adds to the buffer and the tracker, so with it held here no
        // sample of the old position can be added after this clear. Lock order: source, then inner (as everywhere).
        let mut inner = lock(&self.shared.inner);
        inner.buf.clear();
        inner.consumed = 0;
        inner.tracker.reset(0);
        true
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
        self.shared.wake.notify();
        self.shared.cancel.store(true, Ordering::SeqCst);
        if let Some(h) = self.producer.take() {
            let _ = h.join();
        }
    }
}

/// Applies what `push` and `clear_queued` left, in order. The caller holds the source lock.
fn apply_pending(s: &Shared, src: &mut GaplessSource) {
    let changes = std::mem::take(&mut *lock(&s.pending));
    for change in changes {
        match change {
            Change::Push(t) => src.push(*t),
            Change::ClearQueued => src.clear_queued(),
        }
    }
    s.queued_tracks.store(src.queued(), Ordering::Relaxed);
}

fn produce(s: &Shared) {
    let cap = s.rate as usize * CHANNELS * BUFFER_SECS;
    let mut tmp = vec![0f32; CHUNK_SAMPLES];
    while !s.stop.load(Ordering::Relaxed) {
        let seen = s.wake.epoch();
        let free = cap.saturating_sub(lock(&s.inner).buf.len());
        if free < CHUNK_SAMPLES {
            // Nothing to decode yet (the buffer is full, or paused), but queue changes must not wait for room.
            let mut busy = false;
            if !lock(&s.pending).is_empty() {
                match s.source.try_lock() {
                    Ok(mut src) => apply_pending(s, &mut src),
                    Err(_) => busy = true,
                }
            }
            if busy {
                thread::sleep(Duration::from_millis(5));
            } else {
                s.wake
                    .wait_for_space(seen, FULL_LIMIT, Duration::from_millis(5));
            }
            continue;
        }
        let mut src = lock(&s.source);
        apply_pending(s, &mut src);
        let n = src.fill(&mut tmp);
        let events = src.drain_events();
        s.queued_tracks.store(src.queued(), Ordering::Relaxed);
        if s.cancel.load(Ordering::SeqCst) {
            // A flush is waiting for the lock and throws all of this away. A track whose read it just
            // cancelled must not be reported as failed either.
            drop(src);
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        if n == 0 && events.is_empty() {
            drop(src);
            s.wake.wait(seen, IDLE_LIMIT, Duration::from_millis(10));
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
        let cap = s.rate as usize * CHANNELS * BUFFER_SECS;
        s.wake
            .space_freed(cap.saturating_sub(inner.buf.len()), cap / REFILL_DIVISOR);
        out[n..].fill(0.0);
        // Volume: ramp across this buffer so a change never clicks.
        let target = s.volume.gain();
        let from = inner.gain;
        if n > 0 && (from != target || target != 1.0) {
            let frames = (n / CHANNELS) as f32;
            for (i, frame) in out[..n]
                .as_chunks_mut::<CHANNELS>()
                .0
                .iter_mut()
                .enumerate()
            {
                let g = from + (target - from) * ((i + 1) as f32 / frames);
                for sample in frame {
                    *sample *= g;
                }
            }
        }
        // While silent, follow the setting; the ramp is only for changes during continuous sound.
        // (Otherwise the first buffer of the next track would ramp down from full volume: a loud blip.)
        inner.gain = target;
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
