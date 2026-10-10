//! Maps "frames the device has consumed" onto track boundaries, so the UI reflects what the
//! user can hear, not what the decoder has produced. The audio engine's producer registers
//! where each track starts/ends in the cumulative output stream; the device callback reports
//! how many frames it has consumed.

use crate::queue::EntryId;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    /// The first sample of this track has reached the output.
    Audible(EntryId),
    /// The last sample of this track has been heard.
    Ended(EntryId),
}

#[derive(Debug)]
struct Span {
    id: EntryId,
    start: u64,
    end: Option<u64>,
    announced: bool,
}

#[derive(Debug, Default)]
pub struct HeardTracker {
    spans: VecDeque<Span>,
    latency_frames: u64,
    heard: u64,
}

impl HeardTracker {
    /// `latency_frames`: how long after being handed to the device a frame becomes audible.
    pub fn new(latency_frames: u64) -> Self {
        Self {
            latency_frames,
            ..Self::default()
        }
    }

    pub fn set_latency(&mut self, frames: u64) {
        self.latency_frames = frames;
    }

    /// Producer side: `id` begins at cumulative output frame `start_frame`.
    pub fn track_started(&mut self, id: EntryId, start_frame: u64) {
        self.spans.push_back(Span {
            id,
            start: start_frame,
            end: None,
            announced: false,
        });
    }

    /// Producer side: `id` ends (exclusive) at cumulative output frame `end_frame`.
    pub fn track_ended(&mut self, id: EntryId, end_frame: u64) {
        if let Some(s) = self
            .spans
            .iter_mut()
            .rev()
            .find(|s| s.id == id && s.end.is_none())
        {
            s.end = Some(end_frame);
        }
    }

    /// Producer side: `id` stopped producing audio at `at_frame` (decode error, expired URL).
    /// A track that produced nothing is dropped without ever being announced; one that produced
    /// audio ends there, so the tracks queued behind it are not blocked.
    pub fn track_failed(&mut self, id: EntryId, at_frame: u64) {
        let Some(i) = self
            .spans
            .iter()
            .rposition(|s| s.id == id && s.end.is_none())
        else {
            return;
        };
        if self.spans[i].start >= at_frame && !self.spans[i].announced {
            self.spans.remove(i);
        } else {
            self.spans[i].end = Some(at_frame);
        }
    }

    /// Forget everything queued (stop, seek or skip flushed the engine). Counting continues from `consumed`.
    pub fn reset(&mut self, consumed_total: u64) {
        self.spans.clear();
        self.heard = consumed_total.saturating_sub(self.latency_frames);
    }

    /// Device side: `consumed_total` frames have been taken by the device so far.
    pub fn advance(&mut self, consumed_total: u64) -> Vec<Heard> {
        self.heard = consumed_total.saturating_sub(self.latency_frames);
        let mut out = Vec::new();
        while let Some(front) = self.spans.front_mut() {
            if !front.announced && self.heard > front.start {
                front.announced = true;
                out.push(Heard::Audible(front.id));
            }
            match front.end {
                Some(end) if self.heard >= end => {
                    if !front.announced {
                        out.push(Heard::Audible(front.id));
                    }
                    out.push(Heard::Ended(front.id));
                    self.spans.pop_front();
                }
                _ => break,
            }
        }
        out
    }

    /// The audible track and how many frames of it have been heard.
    pub fn position(&self) -> Option<(EntryId, u64)> {
        let f = self.spans.front()?;
        f.announced.then(|| (f.id, self.heard - f.start))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u64) -> EntryId {
        EntryId(n)
    }

    #[test]
    fn nothing_is_audible_until_consumed() {
        let mut t = HeardTracker::new(0);
        t.track_started(id(1), 0);
        t.track_ended(id(1), 1000);
        t.track_started(id(2), 1000);
        t.track_ended(id(2), 2500);
        assert!(t.advance(0).is_empty(), "produced but not consumed");
        assert_eq!(t.advance(480), vec![Heard::Audible(id(1))]);
        assert_eq!(t.position(), Some((id(1), 480)));
    }

    #[test]
    fn boundary_inside_one_callback_reports_end_then_start_in_order() {
        let mut t = HeardTracker::new(0);
        t.track_started(id(1), 0);
        t.track_ended(id(1), 1000);
        t.track_started(id(2), 1000);
        t.advance(900);
        let ev = t.advance(1100);
        assert_eq!(ev, vec![Heard::Ended(id(1)), Heard::Audible(id(2))]);
        assert_eq!(t.position(), Some((id(2), 100)));
    }

    #[test]
    fn output_latency_delays_what_is_reported() {
        let mut t = HeardTracker::new(300);
        t.track_started(id(1), 0);
        assert!(
            t.advance(300).is_empty(),
            "300 frames consumed but still in the device pipeline"
        );
        assert_eq!(t.advance(301), vec![Heard::Audible(id(1))]);
    }

    #[test]
    fn track_still_playing_has_no_end_yet() {
        let mut t = HeardTracker::new(0);
        t.track_started(id(1), 0);
        assert_eq!(t.advance(50), vec![Heard::Audible(id(1))]);
        assert!(t.advance(10_000).is_empty());
        t.track_ended(id(1), 12_000);
        assert_eq!(t.advance(12_000), vec![Heard::Ended(id(1))]);
    }

    #[test]
    fn very_short_track_still_announces_before_ending() {
        let mut t = HeardTracker::new(0);
        t.track_started(id(1), 0);
        t.track_ended(id(1), 0);
        assert_eq!(
            t.advance(10),
            vec![Heard::Audible(id(1)), Heard::Ended(id(1))]
        );
    }

    #[test]
    fn a_track_that_dies_mid_stream_ends_there_and_does_not_block_the_next() {
        let mut t = HeardTracker::new(0);
        t.track_started(id(1), 0);
        t.track_failed(id(1), 500);
        t.track_started(id(2), 500);
        t.track_ended(id(2), 900);
        assert_eq!(t.advance(100), vec![Heard::Audible(id(1))]);
        assert_eq!(
            t.advance(600),
            vec![Heard::Ended(id(1)), Heard::Audible(id(2))]
        );
        assert_eq!(t.advance(900), vec![Heard::Ended(id(2))]);
    }

    #[test]
    fn a_track_that_failed_before_any_audio_is_never_announced() {
        let mut t = HeardTracker::new(0);
        t.track_started(id(1), 0);
        t.track_failed(id(1), 0);
        t.track_started(id(2), 0);
        t.track_ended(id(2), 400);
        assert_eq!(t.advance(10), vec![Heard::Audible(id(2))]);
    }

    #[test]
    fn reset_drops_pending_tracks() {
        let mut t = HeardTracker::new(0);
        t.track_started(id(1), 0);
        t.track_ended(id(1), 1000);
        t.reset(200);
        assert!(t.advance(5000).is_empty());
        assert_eq!(t.position(), None);
    }
}
