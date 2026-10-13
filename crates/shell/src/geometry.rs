//! Remembering a window's place (the mini player) and making sure it is still reachable after monitors changed.

use crate::anchor::clamp_into;
use crate::Rect;

/// A saved window is "still reachable" when at least this much of it (both ways) is on a monitor.
const MIN_VISIBLE: i32 = 80;

/// Returns `saved` if it can still be grabbed, nudged inside the monitor it mostly sits on if it hangs off the edge,
/// or centred on the first monitor (the primary) if it is on none (e.g. an unplugged second screen).
/// `monitors` are work areas in physical pixels.
pub fn fit_on_screen(saved: Rect, monitors: &[Rect]) -> Rect {
    let Some(first) = monitors.first() else {
        return saved;
    };
    let best = monitors
        .iter()
        .filter_map(|m| m.intersection(&saved).map(|i| (m, i)))
        .max_by_key(|(_, i)| i64::from(i.w) * i64::from(i.h));
    match best {
        Some((m, i)) if i.w >= MIN_VISIBLE.min(saved.w) && i.h >= MIN_VISIBLE.min(saved.h) => {
            let (x, y) = clamp_into(*m, (saved.x, saved.y), (saved.w, saved.h), 0);
            Rect::new(x, y, saved.w, saved.h)
        }
        _ => {
            let (cx, cy) = first.center();
            let (x, y) = clamp_into(
                *first,
                (cx - saved.w / 2, cy - saved.h / 2),
                (saved.w, saved.h),
                0,
            );
            Rect::new(x, y, saved.w, saved.h)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMARY: Rect = Rect::new(0, 0, 1920, 1032);
    const LEFT: Rect = Rect::new(-1920, 0, 1920, 1080);

    #[test]
    fn a_window_in_view_stays_exactly_where_it_was() {
        let w = Rect::new(1500, 700, 340, 120);
        assert_eq!(fit_on_screen(w, &[PRIMARY]), w);
    }

    #[test]
    fn a_window_that_hangs_a_little_off_the_edge_is_pulled_inside() {
        let w = Rect::new(1800, 950, 340, 120); // 38 px below and 220 px right of the screen
        assert_eq!(
            fit_on_screen(w, &[PRIMARY]),
            Rect::new(1920 - 340, 1032 - 120, 340, 120)
        );
    }

    #[test]
    fn a_window_with_most_of_its_height_off_screen_counts_as_lost_and_is_centred() {
        let w = Rect::new(1800, 1000, 340, 120); // only 32 px of it visible
        assert_eq!(
            fit_on_screen(w, &[PRIMARY]),
            Rect::new(960 - 170, 516 - 60, 340, 120)
        );
    }

    #[test]
    fn a_window_that_was_on_an_unplugged_monitor_comes_back_centred_on_the_primary() {
        let w = Rect::new(-1500, 300, 340, 120);
        assert_eq!(
            fit_on_screen(w, &[PRIMARY]),
            Rect::new(960 - 170, 516 - 60, 340, 120)
        );
    }

    #[test]
    fn with_the_second_monitor_present_the_saved_place_there_is_kept() {
        let w = Rect::new(-1500, 300, 340, 120);
        assert_eq!(fit_on_screen(w, &[PRIMARY, LEFT]), w);
    }

    #[test]
    fn a_window_straddling_two_monitors_goes_to_the_one_it_mostly_covers() {
        let w = Rect::new(-100, 300, 340, 120); // 100 px on the left screen, 240 on the primary
        assert_eq!(
            fit_on_screen(w, &[PRIMARY, LEFT]),
            Rect::new(0, 300, 340, 120)
        );
    }

    #[test]
    fn a_sliver_on_screen_is_not_reachable_enough() {
        let w = Rect::new(1900, 300, 340, 120); // only 20 px visible
        assert_eq!(
            fit_on_screen(w, &[PRIMARY]),
            Rect::new(960 - 170, 516 - 60, 340, 120)
        );
    }

    #[test]
    fn no_monitor_information_changes_nothing() {
        let w = Rect::new(5, 6, 7, 8);
        assert_eq!(fit_on_screen(w, &[]), w);
    }
}
