//! Turns engine events into "this song was played" for the history.

use pyxee_core::{EntryId, PlayCounter, Track};

#[derive(Default)]
pub struct HistoryRecorder {
    counter: PlayCounter,
    current: Option<(EntryId, Track)>,
}

impl HistoryRecorder {
    /// A track became audible (by what the user can hear, not what was decoded).
    pub fn audible(&mut self, id: EntryId, track: Track) {
        self.counter.started(id, track.duration_ms);
        self.current = Some((id, track));
    }

    /// Returns the track exactly once per listen, when enough of it has really been heard.
    pub fn position(&mut self, id: EntryId, ms: u64) -> Option<Track> {
        if !self.counter.position(id, ms) {
            return None;
        }
        self.current
            .as_ref()
            .filter(|(cur, _)| *cur == id)
            .map(|(_, t)| t.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(n: &str, dur: u64) -> Track {
        let mut t = Track::new(&format!("yt:{n}"), n, "x");
        t.duration_ms = Some(dur);
        t
    }

    fn play(r: &mut HistoryRecorder, id: u64, secs: u64) -> Vec<String> {
        (0..=secs * 4)
            .filter_map(|i| r.position(EntryId(id), i * 250))
            .map(|t| t.key)
            .collect()
    }

    #[test]
    fn a_song_is_recorded_once_after_enough_listening() {
        let mut r = HistoryRecorder::default();
        r.audible(EntryId(1), track("a", 200_000));
        assert_eq!(play(&mut r, 1, 45), ["yt:a"]);
    }

    #[test]
    fn skipped_songs_leave_no_trace() {
        let mut r = HistoryRecorder::default();
        r.audible(EntryId(1), track("a", 200_000));
        assert!(play(&mut r, 1, 8).is_empty());
        r.audible(EntryId(2), track("b", 200_000));
        assert_eq!(play(&mut r, 2, 40), ["yt:b"]);
    }

    #[test]
    fn each_listen_of_a_repeated_song_is_recorded() {
        let mut r = HistoryRecorder::default();
        let mut all = Vec::new();
        for _ in 0..2 {
            r.audible(EntryId(1), track("a", 60_000));
            all.extend(play(&mut r, 1, 35));
        }
        assert_eq!(all, ["yt:a", "yt:a"]);
    }
}
