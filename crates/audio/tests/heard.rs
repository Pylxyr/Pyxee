//! The reported "now playing" must follow what the device has consumed, not what the decoder produced.

use pyxee_audio::decode::SampleSource;
use pyxee_audio::gapless::{Event, GaplessSource, PreparedTrack};
use pyxee_audio::AudioError;
use pyxee_core::{EntryId, Heard, HeardTracker};

struct Tone {
    left: usize,
}

impl SampleSource for Tone {
    fn sample_rate(&self) -> u32 {
        48_000
    }
    fn channels(&self) -> usize {
        2
    }
    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError> {
        if self.left == 0 {
            return Ok(false);
        }
        let n = self.left.min(512);
        self.left -= n;
        out.extend(std::iter::repeat_n(0.25f32, n * 2));
        Ok(true)
    }
}

/// Producer side: forward engine events (with their cumulative frame positions) to the tracker.
fn feed(tracker: &mut HeardTracker, events: Vec<Event>) {
    for e in events {
        match e {
            Event::Started { id, at_frame } => tracker.track_started(EntryId(id), at_frame),
            Event::Finished { id, at_frame, .. } => tracker.track_ended(EntryId(id), at_frame),
            Event::Failed { .. } => {}
        }
    }
}

#[test]
fn track_changes_are_reported_when_heard_not_when_decoded() {
    const A: usize = 48_000; // 1 s
    const B: usize = 72_000; // 1.5 s
    const LATENCY: u64 = 1_440; // 30 ms in the device pipeline

    let mut g = GaplessSource::new();
    g.push(PreparedTrack::new(1, Box::new(Tone { left: A }), 48_000).unwrap());
    g.push(PreparedTrack::new(2, Box::new(Tone { left: B }), 48_000).unwrap());
    let mut tracker = HeardTracker::new(LATENCY);

    // The producer races ahead and decodes both tracks completely before the device plays a sample.
    let mut buf = vec![0f32; 4096];
    let mut produced = 0u64;
    loop {
        let n = g.fill(&mut buf);
        produced += (n / 2) as u64;
        feed(&mut tracker, g.drain_events());
        if n < buf.len() {
            break;
        }
    }
    assert_eq!(produced, (A + B) as u64);
    assert!(
        tracker.advance(0).is_empty(),
        "everything is decoded, nothing is audible yet"
    );

    // The device now consumes 480-frame (10 ms) callbacks.
    let mut consumed = 0u64;
    let mut log: Vec<(u64, Heard)> = Vec::new();
    while consumed < produced + LATENCY {
        consumed += 480;
        for h in tracker.advance(consumed) {
            log.push((consumed, h));
        }
    }

    let at = |h: Heard| log.iter().find(|(_, x)| *x == h).map(|(c, _)| *c).unwrap();
    let a_start = at(Heard::Audible(EntryId(1)));
    let a_end = at(Heard::Ended(EntryId(1)));
    let b_start = at(Heard::Audible(EntryId(2)));
    let b_end = at(Heard::Ended(EntryId(2)));

    // Nothing is announced before it has gone through the output latency...
    assert!(
        a_start > LATENCY && a_start <= LATENCY + 480,
        "A audible at {a_start}"
    );
    // ...A ends and B starts in the same callback (gapless), ~1 s of audio + latency later...
    // B's first sample is only "heard" once the sample after A's last has played, so on an exact
    // callback edge B is announced one callback after A ends (mid-callback they arrive together).
    assert!(
        b_start >= a_end && b_start <= a_end + 480,
        "A ended {a_end}, B audible {b_start}"
    );
    assert!(
        a_end >= A as u64 + LATENCY && a_end < A as u64 + LATENCY + 480,
        "A ended at {a_end}"
    );
    // ...and B ends 1.5 s after that.
    assert!(b_end >= (A + B) as u64 + LATENCY && b_end < (A + B) as u64 + LATENCY + 480);

    // Order within the boundary callback: Ended(A) before Audible(B).
    let ia = log
        .iter()
        .position(|(_, h)| *h == Heard::Ended(EntryId(1)))
        .unwrap();
    let ib = log
        .iter()
        .position(|(_, h)| *h == Heard::Audible(EntryId(2)))
        .unwrap();
    assert!(ia < ib);
}
