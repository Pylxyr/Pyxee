//! Where the tray flyout goes: next to the tray icon, on the side the taskbar is on, inside the work area.

use crate::Rect;

/// A monitor's full area and its work area (the part not covered by the taskbar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Monitor {
    pub area: Rect,
    pub work: Rect,
}

/// The monitor the point is on; when it is on none (a tray icon can sit in the taskbar strip of `area` but outside
/// `work`, so `area` is what counts), the closest one by centre distance.
pub fn monitor_at(monitors: &[Monitor], p: (i32, i32)) -> Option<&Monitor> {
    monitors.iter().find(|m| m.area.contains(p)).or_else(|| {
        monitors.iter().min_by_key(|m| {
            let c = m.area.center();
            let (dx, dy) = (i64::from(c.0 - p.0), i64::from(c.1 - p.1));
            dx * dx + dy * dy
        })
    })
}

/// Top-left corner for a flyout of `size` opened from the tray icon `icon`. The flyout sits against the work area
/// edge nearest the icon (above a bottom taskbar, below a top one, beside a side one), centred on the icon along the
/// taskbar, then clamped so it never leaves the work area. `margin` is the gap to the edge and the icon.
pub fn flyout_position(
    icon: Rect,
    monitors: &[Monitor],
    size: (i32, i32),
    margin: i32,
) -> (i32, i32) {
    let Some(mon) = monitor_at(monitors, icon.center()) else {
        return (icon.x, icon.y);
    };
    let work = mon.work;
    let (cx, cy) = icon.center();
    let (w, h) = size;
    // Which side is the taskbar on? The side where the work area is smaller than the monitor and the icon lies.
    let (x, y) = if cy >= work.bottom() {
        (cx - w / 2, work.bottom() - h - margin)
    } else if cy < work.y {
        (cx - w / 2, work.y + margin)
    } else if cx < work.x {
        (work.x + margin, cy - h / 2)
    } else if cx >= work.right() {
        (work.right() - w - margin, cy - h / 2)
    } else {
        // The icon is inside the work area (an auto-hidden taskbar): open above it.
        (cx - w / 2, icon.y - h - margin)
    };
    clamp_into(work, (x, y), size, margin)
}

/// Clicking the tray icon while the flyout is open first takes focus away from it (so it hides) and then delivers the
/// click. Without this the click would open it again straight away.
pub fn reopen_after_blur(ms_since_hidden: u64) -> bool {
    ms_since_hidden > 250
}

/// Moves `pos` so a window of `size` lies inside `area` with `margin` to spare (as far as it fits).
pub fn clamp_into(area: Rect, pos: (i32, i32), size: (i32, i32), margin: i32) -> (i32, i32) {
    let max_x = (area.right() - size.0 - margin).max(area.x);
    let max_y = (area.bottom() - size.1 - margin).max(area.y);
    (
        pos.0.clamp((area.x + margin).min(max_x), max_x),
        pos.1.clamp((area.y + margin).min(max_y), max_y),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1920x1080 with a 48 px taskbar at the bottom.
    fn bottom_bar() -> Monitor {
        Monitor {
            area: Rect::new(0, 0, 1920, 1080),
            work: Rect::new(0, 0, 1920, 1032),
        }
    }

    #[test]
    fn above_a_bottom_taskbar_centred_on_the_icon() {
        let icon = Rect::new(1700, 1040, 24, 40);
        let (x, y) = flyout_position(icon, &[bottom_bar()], (360, 140), 12);
        assert_eq!((x, y), (1712 - 180, 1032 - 140 - 12));
    }

    #[test]
    fn near_the_right_edge_it_is_pulled_back_inside() {
        let icon = Rect::new(1890, 1040, 24, 40);
        let (x, _) = flyout_position(icon, &[bottom_bar()], (360, 140), 12);
        assert_eq!(x, 1920 - 360 - 12);
    }

    #[test]
    fn below_a_top_taskbar() {
        let m = Monitor {
            area: Rect::new(0, 0, 1920, 1080),
            work: Rect::new(0, 48, 1920, 1032),
        };
        let (x, y) = flyout_position(Rect::new(1000, 4, 24, 40), &[m], (360, 140), 12);
        assert_eq!((x, y), (1012 - 180, 48 + 12));
    }

    #[test]
    fn beside_a_left_or_right_taskbar() {
        let left = Monitor {
            area: Rect::new(0, 0, 1920, 1080),
            work: Rect::new(60, 0, 1860, 1080),
        };
        let (x, y) = flyout_position(Rect::new(10, 900, 40, 24), &[left], (360, 140), 12);
        assert_eq!((x, y), (60 + 12, 912 - 70));
        let right = Monitor {
            area: Rect::new(0, 0, 1920, 1080),
            work: Rect::new(0, 0, 1860, 1080),
        };
        let (x, _) = flyout_position(Rect::new(1870, 900, 40, 24), &[right], (360, 140), 12);
        assert_eq!(x, 1860 - 360 - 12);
    }

    #[test]
    fn a_hidden_taskbar_with_the_icon_inside_the_work_area_opens_above_the_icon() {
        let m = Monitor {
            area: Rect::new(0, 0, 1920, 1080),
            work: Rect::new(0, 0, 1920, 1080),
        };
        let (x, y) = flyout_position(Rect::new(1000, 1056, 24, 24), &[m], (360, 140), 12);
        assert_eq!((x, y), (1012 - 180, 1056 - 140 - 12));
    }

    #[test]
    fn a_second_monitor_to_the_left_with_negative_coordinates() {
        let left = Monitor {
            area: Rect::new(-1920, 0, 1920, 1080),
            work: Rect::new(-1920, 0, 1920, 1032),
        };
        let right = bottom_bar();
        let (x, y) = flyout_position(
            Rect::new(-300, 1040, 24, 40),
            &[right, left],
            (360, 140),
            12,
        );
        assert_eq!((x, y), (-288 - 180, 1032 - 140 - 12));
        // Near that monitor's left edge it stays on that monitor.
        let (x, _) = flyout_position(
            Rect::new(-1915, 1040, 24, 40),
            &[right, left],
            (360, 140),
            12,
        );
        assert_eq!(x, -1920 + 12);
    }

    #[test]
    fn a_window_bigger_than_the_work_area_is_pinned_to_its_corner_not_thrown_out() {
        let (x, y) = flyout_position(
            Rect::new(100, 1040, 24, 40),
            &[bottom_bar()],
            (4000, 3000),
            12,
        );
        assert_eq!((x, y), (0, 0));
    }

    #[test]
    fn a_tray_click_right_after_the_flyout_blurred_does_not_reopen_it() {
        assert!(!reopen_after_blur(0));
        assert!(!reopen_after_blur(250));
        assert!(reopen_after_blur(251));
        assert!(reopen_after_blur(60_000));
    }

    #[test]
    fn the_nearest_monitor_is_used_when_the_icon_is_on_none() {
        let ms = [bottom_bar()];
        assert!(monitor_at(&ms, (5000, 5000)).is_some());
        assert_eq!(monitor_at(&[], (0, 0)), None);
        assert_eq!(
            flyout_position(Rect::new(5, 5, 1, 1), &[], (10, 10), 0),
            (5, 5)
        );
    }
}
