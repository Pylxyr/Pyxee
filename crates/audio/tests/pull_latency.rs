//! How long does the device callback's `pull` take while the producer thread decodes and the engine polls and seeks?
//! The callback holds the pipeline's lock, so a long pull would be a glitch. Run on demand (it takes ~12 s):
//!   cargo test -p pyxee-audio --test pull_latency -- --ignored --nocapture

use pyxee_audio::decode::SampleSource;
use pyxee_audio::gapless::PreparedTrack;
use pyxee_audio::pipeline::{Pipeline, Volume};
use pyxee_audio::AudioError;
use pyxee_core::EntryId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

const RATE: u32 = 48_000;

struct Tone {
    frame: u64,
}

impl SampleSource for Tone {
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn channels(&self) -> usize {
        2
    }
    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError> {
        for f in self.frame..self.frame + 1024 {
            let v = ((f as f32) * 0.05).sin() * 0.1;
            out.extend([v, v]);
        }
        self.frame += 1024;
        Ok(true)
    }
    fn seek_ms(&mut self, ms: u64) -> Result<(), AudioError> {
        self.frame = ms * u64::from(RATE) / 1000;
        Ok(())
    }
}

#[test]
#[ignore = "a measurement, not a check: run it on demand"]
fn the_device_callback_never_waits_long_for_the_lock() {
    let (tx, _rx) = channel();
    let p = Arc::new(Pipeline::new(RATE, tx, Volume::default()));
    let c = p.consumer();
    p.push(PreparedTrack::new(1, Box::new(Tone { frame: 0 }), RATE).unwrap());

    let stop = Arc::new(AtomicBool::new(false));
    // The engine thread: polls the heard position every 20 ms and seeks every half second.
    let engine = {
        let (p, stop) = (Arc::clone(&p), Arc::clone(&stop));
        std::thread::spawn(move || {
            let mut n = 0u64;
            while !stop.load(Ordering::Relaxed) {
                let _ = p.heard_position();
                n += 1;
                if n.is_multiple_of(25) {
                    let _ = p.seek_in_place(EntryId(1), (n * 7) % 60_000);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        })
    };

    let mut buf = vec![0f32; 480 * 2];
    let mut times = Vec::with_capacity(1200);
    let mut silent = 0usize;
    let started = Instant::now();
    let mut next = started;
    while started.elapsed() < Duration::from_secs(12) {
        let t = Instant::now();
        let n = c.pull(&mut buf, 0);
        times.push(t.elapsed());
        if n == 0 {
            silent += 1;
        }
        next += Duration::from_millis(10);
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    stop.store(true, Ordering::Relaxed);
    engine.join().unwrap();

    times.sort();
    let at = |q: f64| times[((times.len() - 1) as f64 * q) as usize];
    println!(
        "pull() over {} callbacks of 10 ms: p50 {:?}, p99 {:?}, p99.9 {:?}, max {:?}; empty pulls {}",
        times.len(),
        at(0.5),
        at(0.99),
        at(0.999),
        times.last().unwrap(),
        silent
    );
    assert!(
        *times.last().unwrap() < Duration::from_millis(5),
        "a pull took half of a callback period"
    );
}
