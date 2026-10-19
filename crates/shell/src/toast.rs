//! When a "song changed" notification may be shown. Windows keeps every toast in the Action Center, so a player that
//! toasts each song while nobody is looking leaves a pile for the listener to find on return. Pure: time is passed in.
//!
//! Rules, in order: nothing while the window is in front (and that counts as the listener being back); never the same
//! song twice in a row (an "audible" event can repeat); at most one toast per `MIN_GAP_MS`; and at most
//! `MAX_UNATTENDED` toasts in a row without the listener coming back to the window. Songs that are not toasted are
//! dropped, never queued for later.

/// Two toasts are at least this far apart.
pub const MIN_GAP_MS: u64 = 15_000;
/// After this many toasts nobody looked at, stop until the window has been in front again.
pub const MAX_UNATTENDED: u32 = 3;

#[derive(Debug, Default, Clone)]
pub struct ToastGate {
    last_id: Option<u64>,
    last_shown_ms: Option<u64>,
    unattended: u32,
}

impl ToastGate {
    /// The listener has the window in front.
    pub fn window_focused(&mut self) {
        self.unattended = 0;
    }

    /// A song became audible: should it get a toast?
    pub fn allow(&mut self, id: u64, now_ms: u64, window_focused: bool) -> bool {
        let repeat = self.last_id == Some(id);
        self.last_id = Some(id);
        if window_focused {
            self.window_focused();
            return false;
        }
        if repeat || self.unattended >= MAX_UNATTENDED {
            return false;
        }
        if self
            .last_shown_ms
            .is_some_and(|t| now_ms.saturating_sub(t) < MIN_GAP_MS)
        {
            return false;
        }
        self.last_shown_ms = Some(now_ms);
        self.unattended += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_song_change_while_away_gets_a_toast() {
        assert!(ToastGate::default().allow(1, 0, false));
    }

    #[test]
    fn nothing_while_the_window_is_in_front() {
        let mut g = ToastGate::default();
        assert!(!g.allow(1, 0, true));
    }

    #[test]
    fn the_same_song_is_not_announced_twice() {
        let mut g = ToastGate::default();
        assert!(g.allow(1, 0, false));
        assert!(
            !g.allow(1, 60_000, false),
            "a repeated event for the same song"
        );
    }

    #[test]
    fn skipping_through_songs_gives_one_toast_not_one_each() {
        let mut g = ToastGate::default();
        assert!(g.allow(1, 0, false));
        for (i, id) in (2..8u64).enumerate() {
            assert!(
                !g.allow(id, 1_000 * (i as u64 + 1), false),
                "song {id} came within the gap"
            );
        }
        assert!(g.allow(9, MIN_GAP_MS + 1, false));
    }

    #[test]
    fn a_night_of_music_leaves_three_toasts_not_a_hundred() {
        let mut g = ToastGate::default();
        let shown = (1..=120u64)
            .filter(|id| g.allow(*id, id * 200_000, false))
            .count();
        assert_eq!(shown, MAX_UNATTENDED as usize);
    }

    #[test]
    fn coming_back_to_the_window_lets_toasts_start_again() {
        let mut g = ToastGate::default();
        for id in 1..=10u64 {
            g.allow(id, id * 200_000, false);
        }
        assert!(!g.allow(11, 11 * 200_000, false));
        g.window_focused();
        assert!(g.allow(12, 12 * 200_000, false));
    }

    #[test]
    fn a_change_while_focused_counts_as_being_back() {
        let mut g = ToastGate::default();
        for id in 1..=3u64 {
            assert!(g.allow(id, id * 100_000, false));
        }
        assert!(!g.allow(4, 400_000, true));
        assert!(g.allow(5, 500_000, false));
    }
}
