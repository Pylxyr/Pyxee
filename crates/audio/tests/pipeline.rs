//! The pipeline end to end with real decoding: heard events follow consumption, flush is instant,
//! pause freezes the clock, and failures are reported without stopping the queue.

use pyxee_audio::decode::{SampleSource, SymphoniaSource};
use pyxee_audio::gap::find_gaps;
use pyxee_audio::gapless::{FailKind, PreparedTrack};
use pyxee_audio::pipeline::{Consumer, PipeEvent, Pipeline};
use pyxee_audio::AudioError;
use pyxee_core::{EntryId, Heard};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::thread::sleep;
use std::time::{Duration, Instant};

const RATE: u32 = 48_000;
const CHUNK: usize = 480; // frames per simulated device callback (10 ms)

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn track(id: u64, file: &str) -> PreparedTrack {
    let src = SymphoniaSource::open_path(&fixture(file)).unwrap();
    PreparedTrack::new(id, Box::new(src), RATE).unwrap()
}

fn setup() -> (Pipeline, Consumer, Receiver<PipeEvent>) {
    let (tx, rx) = channel();
    let p = Pipeline::new(RATE, tx);
    let c = p.consumer();
    (p, c, rx)
}

fn heard(rx: &Receiver<PipeEvent>) -> Vec<Heard> {
    rx.try_iter()
        .filter_map(|e| {
            if let PipeEvent::Heard(h) = e {
                Some(h)
            } else {
                None
            }
        })
        .collect()
}

/// Pulls device-sized chunks until `done` says stop (or 15 s pass), returning only the real samples.
/// When the buffer is momentarily empty it waits for the producer instead of spinning.
fn drive(
    c: &Consumer,
    rx: &Receiver<PipeEvent>,
    log: &mut Vec<Heard>,
    done: impl Fn(&[Heard]) -> bool,
) -> Vec<f32> {
    let mut out = Vec::new();
    let mut buf = vec![0f32; CHUNK * 2];
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        let n = c.pull(&mut buf, 0);
        out.extend_from_slice(&buf[..n]);
        log.extend(heard(rx));
        if done(log) {
            break;
        }
        if n == 0 {
            sleep(Duration::from_millis(1));
        }
    }
    out
}

const A: EntryId = EntryId(1);
const B: EntryId = EntryId(2);

#[test]
fn two_tracks_play_in_order_without_gaps_and_events_follow_consumption() {
    let (p, c, rx) = setup();
    p.push(track(1, "a_44k.wav"));
    p.push(track(2, "b_48k.wav"));
    sleep(Duration::from_millis(200)); // let the producer get ahead: everything is decoded, nothing consumed

    assert!(
        heard(&rx).is_empty(),
        "nothing is audible before the device consumes anything"
    );
    let mut buf = vec![0f32; CHUNK * 2];
    let first = c.pull(&mut buf, 0);
    assert_eq!(
        heard(&rx),
        vec![Heard::Audible(A)],
        "only the first chunk is heard so far"
    );

    let mut log = vec![Heard::Audible(A)];
    let mut samples = buf[..first].to_vec();
    samples.extend(drive(&c, &rx, &mut log, |l| l.contains(&Heard::Ended(B))));

    assert_eq!(
        log,
        vec![
            Heard::Audible(A),
            Heard::Ended(A),
            Heard::Audible(B),
            Heard::Ended(B)
        ]
    );
    // Only real samples were collected, so any silence here is a genuine gap.
    assert_eq!(
        samples.len(),
        2 * 192_000,
        "every frame of both tracks was consumed"
    );
    assert!(
        find_gaps(&samples, 2, 1e-3, 8).is_empty(),
        "silence found between or within tracks"
    );
}

#[test]
fn flush_silences_immediately_and_stale_events_never_arrive() {
    let (p, c, rx) = setup();
    p.push(track(1, "a_44k.wav"));
    sleep(Duration::from_millis(150));
    let mut log = Vec::new();
    drive(&c, &rx, &mut log, |l| l.contains(&Heard::Ended(A))); // consume ~10 s worth of pulls: whole track 1 plays and ends
    log.clear();

    p.push(track(1, "a_44k.wav"));
    sleep(Duration::from_millis(150));
    let mut buf = vec![0f32; CHUNK * 2];
    c.pull(&mut buf, 0);
    assert!(buf.iter().any(|s| s.abs() > 0.01), "audio is playing");

    p.flush();
    c.pull(&mut buf, 0);
    assert!(
        buf.iter().all(|s| *s == 0.0),
        "the very next callback after a flush is silent"
    );

    p.push(track(2, "b_48k.wav"));
    sleep(Duration::from_millis(150));
    let mut log2 = Vec::new();
    rx.try_iter().for_each(drop);
    drive(&c, &rx, &mut log2, |l| l.contains(&Heard::Ended(B)));
    assert_eq!(
        log2,
        vec![Heard::Audible(B), Heard::Ended(B)],
        "no events for the flushed track"
    );
}

#[test]
fn pause_freezes_output_and_the_heard_clock_then_resume_continues() {
    let (p, c, rx) = setup();
    p.push(track(1, "a_44k.wav"));
    sleep(Duration::from_millis(150));
    let mut buf = vec![0f32; CHUNK * 2];
    for _ in 0..20 {
        c.pull(&mut buf, 0); // 200 ms
    }
    let (_, ms_before) = p.heard_position().unwrap();
    assert!((190..=210).contains(&ms_before), "heard {ms_before} ms");

    p.set_paused(true);
    for _ in 0..50 {
        assert_eq!(c.pull(&mut buf, 0), 0);
        assert!(buf.iter().all(|s| *s == 0.0));
    }
    assert_eq!(
        p.heard_position().unwrap().1,
        ms_before,
        "the clock does not run while paused"
    );

    p.set_paused(false);
    c.pull(&mut buf, 0);
    assert!(buf.iter().any(|s| s.abs() > 0.01));
    assert!(p.heard_position().unwrap().1 > ms_before);
    drop(rx);
}

#[test]
fn heard_position_counts_what_was_consumed_not_what_was_decoded() {
    let (p, c, _rx) = setup();
    p.push(track(1, "a_44k.wav"));
    sleep(Duration::from_millis(300)); // ~2 s already decoded into the buffer
    assert_eq!(p.heard_position(), None);
    let mut buf = vec![0f32; CHUNK * 2];
    for _ in 0..50 {
        c.pull(&mut buf, 0); // 500 ms
    }
    let (id, ms) = p.heard_position().unwrap();
    assert_eq!(id, A);
    assert!((490..=510).contains(&ms), "{ms}");
}

#[test]
fn output_latency_delays_what_counts_as_heard() {
    let (_p, c, rx) = setup();
    _p.push(track(1, "a_44k.wav"));
    sleep(Duration::from_millis(150));
    let mut buf = vec![0f32; CHUNK * 2];
    c.pull(&mut buf, 2_400); // 50 ms device latency: 10 ms handed over is not yet audible
    assert!(heard(&rx).is_empty());
    for _ in 0..5 {
        c.pull(&mut buf, 2_400);
    }
    assert_eq!(heard(&rx), vec![Heard::Audible(A)]);
}

/// Yields a little audio, then fails like a rejected stream URL would.
struct DiesWithExpiredUrl {
    reads: u32,
}

impl SampleSource for DiesWithExpiredUrl {
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn channels(&self) -> usize {
        2
    }
    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError> {
        self.reads += 1;
        if self.reads > 40 {
            return Err(AudioError::UrlExpired(403));
        }
        out.extend(std::iter::repeat_n(0.2f32, 1024));
        Ok(true)
    }
}

#[test]
fn a_failing_track_is_reported_with_its_kind_and_the_next_track_still_plays() {
    let (p, c, rx) = setup();
    p.push(PreparedTrack::new(1, Box::new(DiesWithExpiredUrl { reads: 0 }), RATE).unwrap());
    p.push(track(2, "b_48k.wav"));
    sleep(Duration::from_millis(300));
    let mut failed = None;
    let mut log = Vec::new();
    let mut buf = vec![0f32; CHUNK * 2];
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline && !log.contains(&Heard::Ended(B)) {
        if c.pull(&mut buf, 0) == 0 {
            sleep(Duration::from_millis(1));
        }
        for e in rx.try_iter() {
            match e {
                PipeEvent::Heard(h) => log.push(h),
                PipeEvent::Failed { id, kind, .. } => failed = Some((id, kind)),
            }
        }
    }
    assert_eq!(failed, Some((A, FailKind::Expired(403))));
    assert!(
        log.contains(&Heard::Audible(B)) && log.contains(&Heard::Ended(B)),
        "{log:?}"
    );
    drop(p);
}
