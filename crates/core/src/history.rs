//! Decides when a play counts toward history: after the listener has really heard half the song or
//! 30 seconds of it, whichever is less. Skipping through tracks, or seeking to the end, does not count.

use crate::queue::EntryId;

/// Longest listening needed for a play to count.
pub const COUNT_AFTER_MS: u64 = 30_000;
/// Position jumps bigger than this between updates are seeks, not listening.
const MAX_STEP_MS: u64 = 2_000;

struct Listen {
    id: EntryId,
    needed_ms: u64,
    heard_ms: u64,
    last_pos: Option<u64>,
    counted: bool,
}

#[derive(Default)]
pub struct PlayCounter {
    current: Option<Listen>,
}

impl PlayCounter {
    /// A track became audible: a new listen begins (a repeat of the same track is a new listen too).
    pub fn started(&mut self, id: EntryId, duration_ms: Option<u64>) {
        let needed_ms = match duration_ms {
            Some(d) if d > 0 => (d / 2).min(COUNT_AFTER_MS),
            _ => COUNT_AFTER_MS,
        };
        self.current = Some(Listen {
            id,
            needed_ms,
            heard_ms: 0,
            last_pos: None,
            counted: false,
        });
    }

    /// Feed position updates; returns true exactly once per listen, when the play should be recorded.
    pub fn position(&mut self, id: EntryId, ms: u64) -> bool {
        let Some(l) = self.current.as_mut().filter(|l| l.id == id) else {
            return false;
        };
        if let Some(last) = l.last_pos {
            let step = ms.saturating_sub(last);
            if ms >= last && step <= MAX_STEP_MS {
                l.heard_ms += step;
            }
        }
        l.last_pos = Some(ms);
        if !l.counted && l.heard_ms >= l.needed_ms {
            l.counted = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u64) -> EntryId {
        EntryId(n)
    }

    /// Plays `secs` of audio with a position update every 250 ms.
    fn listen(c: &mut PlayCounter, track: u64, from_ms: u64, secs: u64) -> usize {
        (1..=secs * 4)
            .filter(|i| c.position(id(track), from_ms + i * 250))
            .count()
    }

    #[test]
    fn counts_once_after_thirty_seconds() {
        let mut c = PlayCounter::default();
        c.started(id(1), Some(200_000));
        c.position(id(1), 0);
        assert_eq!(listen(&mut c, 1, 0, 29), 0);
        assert_eq!(
            listen(&mut c, 1, 29_000, 20),
            1,
            "counted once, not repeatedly"
        );
    }

    #[test]
    fn short_songs_count_at_half_their_length() {
        let mut c = PlayCounter::default();
        c.started(id(1), Some(20_000));
        c.position(id(1), 0);
        assert_eq!(listen(&mut c, 1, 0, 9), 0);
        assert_eq!(listen(&mut c, 1, 9_000, 2), 1);
    }

    #[test]
    fn unknown_length_uses_the_thirty_second_rule() {
        let mut c = PlayCounter::default();
        c.started(id(1), None);
        c.position(id(1), 0);
        assert_eq!(listen(&mut c, 1, 0, 29), 0);
        assert_eq!(listen(&mut c, 1, 29_000, 2), 1);
    }

    #[test]
    fn seeking_ahead_is_not_listening() {
        let mut c = PlayCounter::default();
        c.started(id(1), Some(300_000));
        c.position(id(1), 0);
        assert!(
            !c.position(id(1), 250_000),
            "jumping to the end earns nothing"
        );
        assert_eq!(listen(&mut c, 1, 250_000, 20), 0);
        assert_eq!(
            listen(&mut c, 1, 255_000, 15),
            1,
            "30 s of real listening after the jump counts"
        );
    }

    #[test]
    fn skipping_before_the_threshold_never_counts() {
        let mut c = PlayCounter::default();
        c.started(id(1), Some(200_000));
        c.position(id(1), 0);
        assert_eq!(listen(&mut c, 1, 0, 10), 0);
        c.started(id(2), Some(200_000));
        c.position(id(2), 0);
        assert_eq!(listen(&mut c, 2, 0, 31), 1);
        assert!(
            !c.position(id(1), 40_000),
            "late updates for the skipped track are ignored"
        );
    }

    #[test]
    fn repeating_a_song_counts_each_listen() {
        let mut c = PlayCounter::default();
        for _ in 0..3 {
            c.started(id(1), Some(40_000));
            c.position(id(1), 0);
            assert_eq!(listen(&mut c, 1, 0, 25), 1);
        }
    }

    #[test]
    fn updates_before_any_track_started_are_ignored() {
        let mut c = PlayCounter::default();
        assert!(!c.position(id(1), 5_000));
    }
}
