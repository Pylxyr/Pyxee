//! What the taskbar button's progress bar shows.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    /// Resolving or buffering a song.
    Loading,
    Playing,
    Paused,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarProgress {
    None,
    Indeterminate,
    /// Green bar, 0..=100.
    Normal(u8),
    /// Yellow bar (paused), 0..=100.
    Paused(u8),
    /// Red bar: the song failed.
    Error,
}

pub fn taskbar_progress(
    phase: Phase,
    position_ms: u64,
    duration_ms: Option<u64>,
) -> TaskbarProgress {
    let percent = || match duration_ms {
        Some(d) if d > 0 => Some((position_ms.min(d) * 100 / d) as u8),
        _ => None,
    };
    match phase {
        Phase::Idle => TaskbarProgress::None,
        Phase::Loading => TaskbarProgress::Indeterminate,
        Phase::Failed => TaskbarProgress::Error,
        Phase::Playing => percent().map_or(TaskbarProgress::Indeterminate, TaskbarProgress::Normal),
        Phase::Paused => percent().map_or(TaskbarProgress::Paused(0), TaskbarProgress::Paused),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playing_shows_a_percentage_and_never_more_than_a_hundred() {
        assert_eq!(
            taskbar_progress(Phase::Playing, 0, Some(200_000)),
            TaskbarProgress::Normal(0)
        );
        assert_eq!(
            taskbar_progress(Phase::Playing, 50_000, Some(200_000)),
            TaskbarProgress::Normal(25)
        );
        assert_eq!(
            taskbar_progress(Phase::Playing, 999_999, Some(200_000)),
            TaskbarProgress::Normal(100)
        );
    }

    #[test]
    fn an_unknown_length_is_indeterminate_while_playing() {
        assert_eq!(
            taskbar_progress(Phase::Playing, 5_000, None),
            TaskbarProgress::Indeterminate
        );
        assert_eq!(
            taskbar_progress(Phase::Playing, 5_000, Some(0)),
            TaskbarProgress::Indeterminate
        );
    }

    #[test]
    fn the_other_phases() {
        assert_eq!(
            taskbar_progress(Phase::Idle, 1, Some(2)),
            TaskbarProgress::None
        );
        assert_eq!(
            taskbar_progress(Phase::Loading, 1, Some(2)),
            TaskbarProgress::Indeterminate
        );
        assert_eq!(
            taskbar_progress(Phase::Failed, 1, Some(2)),
            TaskbarProgress::Error
        );
        assert_eq!(
            taskbar_progress(Phase::Paused, 100_000, Some(200_000)),
            TaskbarProgress::Paused(50)
        );
        assert_eq!(
            taskbar_progress(Phase::Paused, 100_000, None),
            TaskbarProgress::Paused(0)
        );
    }
}
