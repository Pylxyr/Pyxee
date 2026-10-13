//! What the taskbar thumbnail toolbar and the jump list offer, as plain data. The Windows API calls that show them
//! live in the app crate (`shell/winextras.rs`); everything decidable is decided here.

use crate::cli::CliAction;
use crate::progress::Phase;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbButton {
    Previous,
    PlayPause,
    Next,
}

impl ThumbButton {
    pub const ALL: [ThumbButton; 3] = [
        ThumbButton::Previous,
        ThumbButton::PlayPause,
        ThumbButton::Next,
    ];

    /// The id Windows reports back when the button is clicked.
    pub const fn id(self) -> u32 {
        match self {
            ThumbButton::Previous => 100,
            ThumbButton::PlayPause => 101,
            ThumbButton::Next => 102,
        }
    }

    pub fn from_id(id: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.id() == id)
    }

    pub const fn action(self) -> CliAction {
        match self {
            ThumbButton::Previous => CliAction::Previous,
            ThumbButton::PlayPause => CliAction::PlayPause,
            ThumbButton::Next => CliAction::Next,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Previous,
    Play,
    Pause,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThumbSpec {
    pub button: ThumbButton,
    pub glyph: Glyph,
    pub tooltip: &'static str,
    pub enabled: bool,
}

/// The three thumbnail buttons for the current state: play shows a pause glyph while playing, and with nothing
/// loaded the buttons are greyed out.
pub fn thumb_specs(phase: Phase, has_track: bool) -> [ThumbSpec; 3] {
    let playing = phase == Phase::Playing;
    [
        ThumbSpec {
            button: ThumbButton::Previous,
            glyph: Glyph::Previous,
            tooltip: "Previous",
            enabled: has_track,
        },
        ThumbSpec {
            button: ThumbButton::PlayPause,
            glyph: if playing { Glyph::Pause } else { Glyph::Play },
            tooltip: if playing { "Pause" } else { "Play" },
            enabled: has_track,
        },
        ThumbSpec {
            button: ThumbButton::Next,
            glyph: Glyph::Next,
            tooltip: "Next",
            enabled: has_track,
        },
    ]
}

/// A jump-list task: starting Pyxee with `args` (a second launch hands them to the running player).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JumpTask {
    pub title: &'static str,
    pub args: &'static str,
    pub description: &'static str,
}

pub fn jump_tasks() -> [JumpTask; 4] {
    [
        JumpTask {
            title: "Play / Pause",
            args: "--toggle",
            description: "Start or pause the music",
        },
        JumpTask {
            title: "Next song",
            args: "--next",
            description: "Skip to the next song",
        },
        JumpTask {
            title: "Previous song",
            args: "--prev",
            description: "Go back to the previous song",
        },
        JumpTask {
            title: "Mini player",
            args: "--mini",
            description: "Open the always-on-top mini player",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::parse_args;

    #[test]
    fn button_ids_round_trip_and_unknown_ids_are_ignored() {
        for b in ThumbButton::ALL {
            assert_eq!(ThumbButton::from_id(b.id()), Some(b));
        }
        assert_eq!(ThumbButton::from_id(0), None);
        assert_eq!(ThumbButton::from_id(103), None);
        let ids: Vec<u32> = ThumbButton::ALL.iter().map(|b| b.id()).collect();
        assert_eq!(ids, vec![100, 101, 102]);
    }

    #[test]
    fn clicking_a_button_does_what_its_label_says() {
        assert_eq!(ThumbButton::Previous.action(), CliAction::Previous);
        assert_eq!(ThumbButton::PlayPause.action(), CliAction::PlayPause);
        assert_eq!(ThumbButton::Next.action(), CliAction::Next);
    }

    #[test]
    fn the_play_button_shows_pause_while_playing_and_play_otherwise() {
        let playing = thumb_specs(Phase::Playing, true);
        assert_eq!(
            (playing[1].glyph, playing[1].tooltip),
            (Glyph::Pause, "Pause")
        );
        for phase in [Phase::Paused, Phase::Idle, Phase::Loading, Phase::Failed] {
            let s = thumb_specs(phase, true);
            assert_eq!(
                (s[1].glyph, s[1].tooltip),
                (Glyph::Play, "Play"),
                "{phase:?}"
            );
        }
    }

    #[test]
    fn with_nothing_loaded_every_button_is_disabled() {
        assert!(thumb_specs(Phase::Idle, false).iter().all(|s| !s.enabled));
        assert!(thumb_specs(Phase::Playing, true).iter().all(|s| s.enabled));
    }

    #[test]
    fn every_jump_task_is_a_command_line_the_running_player_understands() {
        let expected = [
            CliAction::PlayPause,
            CliAction::Next,
            CliAction::Previous,
            CliAction::Mini,
        ];
        for (task, action) in jump_tasks().iter().zip(expected) {
            let cli = parse_args(&["pyxee.exe", task.args]);
            assert_eq!(cli.actions, vec![action], "{}", task.title);
            assert!(!task.title.is_empty() && !task.description.is_empty());
        }
    }
}
