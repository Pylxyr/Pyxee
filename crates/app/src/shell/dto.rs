//! UI-facing types for the shell features (generated TypeScript lives in `ui/src/lib/bindings`).

use pyxee_lyrics::Lyrics;
use pyxee_shell::settings::{EqSettings, ShellSettings};
use pyxee_shell::sleep::{SleepSetting, SleepStatus};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LyricLineDto {
    #[ts(type = "number")]
    pub time_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum LyricsDto {
    Synced {
        lines: Vec<LyricLineDto>,
    },
    Plain {
        text: String,
    },
    Instrumental,
    /// Looked up, nothing there.
    NotFound,
    /// Could not look it up (offline, service error): worth trying again.
    Error {
        message: String,
    },
}

impl From<Lyrics> for LyricsDto {
    fn from(l: Lyrics) -> Self {
        match l {
            Lyrics::Synced(lines) => Self::Synced {
                lines: lines
                    .into_iter()
                    .map(|l| LyricLineDto {
                        time_ms: l.time_ms,
                        text: l.text,
                    })
                    .collect(),
            },
            Lyrics::Plain(text) => Self::Plain { text },
            Lyrics::Instrumental => Self::Instrumental,
            Lyrics::None => Self::NotFound,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum SleepSettingDto {
    Minutes { minutes: u32 },
    EndOfTrack,
    Tracks { count: u32 },
}

impl From<SleepSettingDto> for SleepSetting {
    fn from(s: SleepSettingDto) -> Self {
        match s {
            SleepSettingDto::Minutes { minutes } => SleepSetting::Minutes { minutes },
            SleepSettingDto::EndOfTrack => SleepSetting::EndOfTrack,
            SleepSettingDto::Tracks { count } => SleepSetting::Tracks { count },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum SleepStatusDto {
    Minutes {
        // serde's rename_all on an enum renames the variants, not their fields.
        #[serde(rename = "remainingMs")]
        #[ts(type = "number", rename = "remainingMs")]
        remaining_ms: u64,
    },
    EndOfTrack,
    Tracks {
        left: u32,
    },
}

impl From<SleepStatus> for SleepStatusDto {
    fn from(s: SleepStatus) -> Self {
        match s {
            SleepStatus::Minutes { remaining_ms } => Self::Minutes { remaining_ms },
            SleepStatus::EndOfTrack => Self::EndOfTrack,
            SleepStatus::Tracks { left } => Self::Tracks { left },
        }
    }
}

/// The `sleep-state` event and the answer to `sleep_get`: `null` status = no timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SleepStateDto {
    pub status: Option<SleepStatusDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShellSettingsDto {
    pub close_to_tray: bool,
    pub start_with_windows: bool,
    pub toast_on_change: bool,
    pub tray_controls: bool,
    pub flyout_on_click: bool,
    pub discord_presence: bool,
    pub lastfm_scrobbling: bool,
    pub check_prereleases: bool,
}

impl From<&ShellSettings> for ShellSettingsDto {
    fn from(s: &ShellSettings) -> Self {
        Self {
            close_to_tray: s.close_to_tray,
            start_with_windows: s.start_with_windows,
            toast_on_change: s.toast_on_change,
            tray_controls: s.tray_controls,
            flyout_on_click: s.flyout_on_click,
            discord_presence: s.discord_presence,
            lastfm_scrobbling: s.lastfm_scrobbling,
            check_prereleases: s.check_prereleases,
        }
    }
}

/// The equalizer: on or off, the preamp and one gain per band (lowest first), in tenths of a dB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EqDto {
    pub enabled: bool,
    pub preamp: i32,
    pub gains: Vec<i32>,
}

impl From<&EqSettings> for EqDto {
    fn from(s: &EqSettings) -> Self {
        Self {
            enabled: s.enabled,
            preamp: s.preamp,
            gains: s.gains.clone(),
        }
    }
}

impl From<EqDto> for EqSettings {
    /// What the window sends is not trusted: the result is always inside what the equalizer can take.
    fn from(d: EqDto) -> Self {
        Self {
            enabled: d.enabled,
            preamp: d.preamp,
            gains: d.gains,
        }
        .sanitized()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_equalizer_limits_agree_between_the_settings_and_the_audio_crate() {
        assert_eq!(pyxee_shell::settings::EQ_BANDS, pyxee_audio::eq::BANDS);
        assert_eq!(
            pyxee_shell::settings::EQ_LIMIT,
            pyxee_audio::eq::LIMIT_TENTHS
        );
    }

    #[test]
    fn what_the_window_sends_for_the_equalizer_is_made_safe() {
        let s: EqSettings = EqDto {
            enabled: true,
            preamp: i32::MAX,
            gains: vec![i32::MIN, 5],
        }
        .into();
        assert_eq!(s.preamp, pyxee_shell::settings::EQ_LIMIT);
        assert_eq!(s.gains.len(), pyxee_shell::settings::EQ_BANDS);
        assert_eq!(&s.gains[..3], &[-pyxee_shell::settings::EQ_LIMIT, 5, 0]);
        assert_eq!(EqDto::from(&s).gains, s.gains);
    }

    #[test]
    fn lyrics_cross_the_boundary_with_their_kind() {
        let dto: LyricsDto = Lyrics::Synced(vec![pyxee_lyrics::Line {
            time_ms: 5,
            text: "la".into(),
        }])
        .into();
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            serde_json::json!({"kind":"synced","lines":[{"timeMs":5,"text":"la"}]})
        );
        assert_eq!(
            serde_json::to_value(LyricsDto::from(Lyrics::None)).unwrap(),
            serde_json::json!({"kind":"notFound"})
        );
        assert_eq!(
            serde_json::to_value(LyricsDto::from(Lyrics::Instrumental)).unwrap(),
            serde_json::json!({"kind":"instrumental"})
        );
    }

    #[test]
    fn sleep_types_use_camel_case_and_a_kind_tag() {
        let s: SleepSettingDto =
            serde_json::from_str(r#"{"kind":"minutes","minutes":30}"#).unwrap();
        assert_eq!(SleepSetting::from(s), SleepSetting::Minutes { minutes: 30 });
        let eot: SleepSettingDto = serde_json::from_str(r#"{"kind":"endOfTrack"}"#).unwrap();
        assert_eq!(SleepSetting::from(eot), SleepSetting::EndOfTrack);
        let st = SleepStateDto {
            status: Some(SleepStatusDto::Minutes { remaining_ms: 1500 }),
        };
        assert_eq!(
            serde_json::to_value(st).unwrap(),
            serde_json::json!({"status":{"kind":"minutes","remainingMs":1500}})
        );
        assert_eq!(
            serde_json::to_value(SleepStateDto { status: None }).unwrap(),
            serde_json::json!({"status":null})
        );
    }
}
