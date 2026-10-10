//! cpal output (WASAPI shared mode on Windows) fed by a producer thread through a ring buffer.
//! The producer decodes; the audio callback only copies, so decoding never blocks the device.

use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use pyxee_audio::gap::Gap;
use pyxee_audio::gapless::{Event, GaplessSource, PreparedTrack};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub fn default_rate() -> Result<u32> {
    let dev = cpal::default_host()
        .default_output_device()
        .context("no default output device")?;
    Ok(dev.default_output_config()?.sample_rate().0)
}

#[derive(Default)]
struct Monitor {
    frames: u64,
    run_start: Option<u64>,
    gaps: Vec<Gap>,
}

impl Monitor {
    fn feed(&mut self, samples: &[f32]) {
        for f in samples.chunks_exact(2) {
            let quiet = f[0].abs() < 1e-3 && f[1].abs() < 1e-3;
            match (quiet, self.run_start) {
                (true, None) => self.run_start = Some(self.frames),
                (false, Some(s)) => {
                    if self.frames - s >= 8 {
                        self.gaps.push(Gap {
                            start_frame: s as usize,
                            len_frames: (self.frames - s) as usize,
                        });
                    }
                    self.run_start = None;
                }
                _ => {}
            }
            self.frames += 1;
        }
    }
}

pub struct Report {
    pub frames: u64,
    pub underruns: u64,
    pub events: Vec<Event>,
    pub gaps: Vec<Gap>,
}

pub struct Player {
    _stream: cpal::Stream,
    source: Arc<Mutex<GaplessSource>>,
    producer: Option<JoinHandle<Monitor>>,
    consumed_frames: Arc<AtomicU64>,
    produced_frames: Arc<AtomicU64>,
    producer_done: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    events: Arc<Mutex<Vec<Event>>>,
    rate: u32,
}

impl Player {
    pub fn start(source: GaplessSource, rate: u32) -> Result<Self> {
        let device = cpal::default_host()
            .default_output_device()
            .context("no default output device")?;
        let cfg = device.default_output_config()?;
        if cfg.sample_format() != SampleFormat::F32 {
            bail!("device default format is {:?}; the spike expects f32 (WASAPI shared mode normally is)", cfg.sample_format());
        }
        if cfg.sample_rate().0 != rate {
            bail!("device rate changed between calls");
        }
        let out_ch = cfg.channels() as usize;

        let rb = HeapRb::<f32>::new(rate as usize * 2 * 2); // ~2 s of stereo
        let (mut prod, mut cons) = rb.split();
        let consumed = Arc::new(AtomicU64::new(0));
        let underruns = Arc::new(AtomicU64::new(0));
        let (c2, u2) = (consumed.clone(), underruns.clone());
        // Pre-size so the audio callback does not allocate for typical buffer sizes.
        let mut scratch: Vec<f32> = Vec::with_capacity(16 * 1024);
        let stream = device.build_output_stream(
            &cfg.config(),
            move |data: &mut [f32], _| {
                let frames = data.len() / out_ch;
                scratch.resize(frames * 2, 0.0);
                let got = cons.pop_slice(&mut scratch);
                if got < scratch.len() {
                    scratch[got..].fill(0.0);
                    u2.fetch_add(1, Ordering::Relaxed);
                }
                for (i, frame) in data.chunks_exact_mut(out_ch).enumerate() {
                    frame.fill(0.0);
                    frame[0] = scratch[i * 2];
                    if out_ch > 1 {
                        frame[1] = scratch[i * 2 + 1];
                    }
                }
                c2.fetch_add((got / 2) as u64, Ordering::Relaxed);
            },
            |e| eprintln!("audio stream error: {e}"),
            None,
        )?;

        let source = Arc::new(Mutex::new(source));
        let produced = Arc::new(AtomicU64::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let closed = Arc::new(AtomicBool::new(false));
        let events = Arc::new(Mutex::new(Vec::new()));
        let (s2, p2, d2, e2, cl) = (
            source.clone(),
            produced.clone(),
            done.clone(),
            events.clone(),
            closed.clone(),
        );
        let producer = thread::spawn(move || {
            let mut mon = Monitor::default();
            let mut buf = vec![0f32; 4096];
            loop {
                if prod.vacant_len() < buf.len() {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                let (n, ev) = {
                    let mut g = s2.lock().unwrap();
                    let n = g.fill(&mut buf);
                    (n, g.drain_events())
                };
                e2.lock().unwrap().extend(ev);
                if n > 0 {
                    mon.feed(&buf[..n]);
                    prod.push_slice(&buf[..n]);
                    p2.fetch_add((n / 2) as u64, Ordering::Relaxed);
                } else {
                    // Queue empty: stop only once the caller says no more tracks are coming.
                    if cl.load(Ordering::Acquire) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
            d2.store(true, Ordering::Release);
            mon
        });

        stream.play()?;
        Ok(Self {
            _stream: stream,
            source,
            producer: Some(producer),
            consumed_frames: consumed,
            produced_frames: produced,
            producer_done: done,
            closed,
            underruns,
            events,
            rate,
        })
    }

    pub fn push(&mut self, t: PreparedTrack) {
        self.source.lock().unwrap().push(t);
    }

    /// Declares that no more tracks will be queued, so playback can end after the last one.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    /// Seconds of audio the device has actually consumed (not wall-clock time).
    pub fn played_secs(&self) -> f64 {
        self.consumed_frames.load(Ordering::Relaxed) as f64 / f64::from(self.rate)
    }

    pub fn wait_until_done(&self) {
        while !(self.producer_done.load(Ordering::Acquire)
            && self.consumed_frames.load(Ordering::Relaxed)
                >= self.produced_frames.load(Ordering::Relaxed))
        {
            thread::sleep(Duration::from_millis(50));
        }
        thread::sleep(Duration::from_millis(300));
    }

    pub fn report(&mut self) -> Report {
        let mon = self
            .producer
            .take()
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default();
        Report {
            frames: mon.frames,
            underruns: self.underruns.load(Ordering::Relaxed),
            events: self.events.lock().unwrap().clone(),
            gaps: mon.gaps,
        }
    }
}
