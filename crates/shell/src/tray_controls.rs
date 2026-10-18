//! The three transport buttons that can sit in the Windows notification area (previous, play/pause, next).
//!
//! Windows has no supported API for a toolbar "docked" next to the overflow chevron: deskbands are gone from the
//! Windows 11 taskbar and unsupported on Windows 10's. What every Windows version does support is several tray
//! icons, so each button is one small icon of its own. This module decides what they show; `app/shell/tray_buttons.rs`
//! only puts that on screen.

use crate::progress::Phase;

/// Longest tooltip Windows shows for a tray icon (127 characters plus the terminator).
pub const TOOLTIP_MAX_CHARS: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Previous,
    PlayPause,
    Next,
}

/// In the order the buttons should read, left to right.
pub const CONTROLS: [Control; 3] = [Control::Previous, Control::PlayPause, Control::Next];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Previous,
    Play,
    Pause,
    Next,
}

impl Control {
    /// The tray icon id, stable across runs.
    pub const fn id(self) -> &'static str {
        match self {
            Control::Previous => "ctl-previous",
            Control::PlayPause => "ctl-playpause",
            Control::Next => "ctl-next",
        }
    }

    pub fn from_id(id: &str) -> Option<Control> {
        CONTROLS.into_iter().find(|c| c.id() == id)
    }

    /// Which picture the button shows. Play/pause shows what pressing it will do: a song that is playing or still
    /// loading offers Pause, anything else offers Play.
    pub const fn glyph(self, phase: Phase) -> Glyph {
        match self {
            Control::Previous => Glyph::Previous,
            Control::Next => Glyph::Next,
            Control::PlayPause => match phase {
                Phase::Playing | Phase::Loading => Glyph::Pause,
                Phase::Idle | Phase::Paused | Phase::Failed => Glyph::Play,
            },
        }
    }

    pub const fn tooltip(self, phase: Phase) -> &'static str {
        match self.glyph(phase) {
            Glyph::Previous => "Previous",
            Glyph::Next => "Next",
            Glyph::Pause => "Pause",
            Glyph::Play => "Play",
        }
    }
}

/// "Title – Artist" for the main icon's tooltip, cut to what Windows will show (never in the middle of a character).
pub fn track_tooltip(title: &str, artist: &str) -> String {
    let text = if artist.trim().is_empty() {
        title.trim().to_string()
    } else {
        format!("{} – {}", title.trim(), artist.trim())
    };
    if text.chars().count() <= TOOLTIP_MAX_CHARS {
        return text;
    }
    let mut cut: String = text.chars().take(TOOLTIP_MAX_CHARS - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_buttons_read_previous_play_next() {
        assert_eq!(
            CONTROLS.map(|c| c.glyph(Phase::Paused)),
            [Glyph::Previous, Glyph::Play, Glyph::Next]
        );
    }

    #[test]
    fn play_pause_offers_what_pressing_it_will_do() {
        for (phase, glyph) in [
            (Phase::Playing, Glyph::Pause),
            (Phase::Loading, Glyph::Pause),
            (Phase::Paused, Glyph::Play),
            (Phase::Idle, Glyph::Play),
            (Phase::Failed, Glyph::Play),
        ] {
            assert_eq!(Control::PlayPause.glyph(phase), glyph, "{phase:?}");
        }
        assert_eq!(Control::PlayPause.tooltip(Phase::Playing), "Pause");
        assert_eq!(Control::PlayPause.tooltip(Phase::Paused), "Play");
        assert_eq!(Control::Previous.tooltip(Phase::Playing), "Previous");
    }

    #[test]
    fn ids_are_unique_and_round_trip() {
        for c in CONTROLS {
            assert_eq!(Control::from_id(c.id()), Some(c));
        }
        let mut ids: Vec<_> = CONTROLS.iter().map(|c| c.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 3);
        assert_eq!(
            Control::from_id("main"),
            None,
            "the main tray icon is not a control"
        );
    }

    #[test]
    fn track_tooltips_join_title_and_artist() {
        assert_eq!(
            track_tooltip("Paper Planes", "Alma Vey"),
            "Paper Planes – Alma Vey"
        );
        assert_eq!(track_tooltip(" Solo ", "  "), "Solo");
    }

    #[test]
    fn long_tooltips_are_cut_on_a_character_boundary() {
        let t = track_tooltip(&"é".repeat(200), "Artist");
        assert_eq!(t.chars().count(), TOOLTIP_MAX_CHARS);
        assert!(t.ends_with('…'));
    }
}
