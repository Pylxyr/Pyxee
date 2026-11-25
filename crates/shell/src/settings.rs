//! Shell settings kept in `shell.json`: tolerant to a missing, damaged or newer file, versioned (see [`crate::schema`])
//! and written atomically.

use crate::Rect;
use crate::schema::{self, Parsed};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MINI_MIN: (i32, i32) = (260, 90);
pub const MINI_MAX: (i32, i32) = (900, 400);

/// The equalizer has this many bands (the audio crate's `eq::BANDS`; a test in the app crate keeps them equal).
pub const EQ_BANDS: usize = 10;
/// Gains and the preamp stay within this many tenths of a dB either way (the audio crate's `eq::LIMIT_TENTHS`).
pub const EQ_LIMIT: i32 = 120;

/// The equalizer as the listener left it. Whole numbers in tenths of a dB, so the settings stay comparable and the
/// file stays tidy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EqSettings {
    pub enabled: bool,
    /// Tenths of a dB.
    pub preamp: i32,
    /// One per band, lowest first, in tenths of a dB. A file with another count is mended by `sanitized`.
    pub gains: Vec<i32>,
}

impl Default for EqSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            preamp: 0,
            gains: vec![0; EQ_BANDS],
        }
    }
}

impl EqSettings {
    /// Exactly `EQ_BANDS` gains (missing ones 0, extra ones dropped), everything within `EQ_LIMIT`.
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        self.preamp = self.preamp.clamp(-EQ_LIMIT, EQ_LIMIT);
        self.gains.resize(EQ_BANDS, 0);
        for g in &mut self.gains {
            *g = (*g).clamp(-EQ_LIMIT, EQ_LIMIT);
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ShellSettings {
    /// The window's close button hides to the tray instead of quitting.
    pub close_to_tray: bool,
    pub start_with_windows: bool,
    /// A notification when the song changes while the window is not focused.
    pub toast_on_change: bool,
    /// Previous / play-pause / next as three small icons in the notification area (beside the main tray icon).
    pub tray_controls: bool,
    /// A left click on the main tray icon opens the flyout (title, artist, progress, buttons). When off it opens
    /// the main window instead. The right-click menu is always there.
    pub flyout_on_click: bool,
    /// Discord Rich Presence ("Listening to Pyxee"). Only does anything when the build has a Discord application
    /// ID baked in (`PYXEE_DISCORD_CLIENT_ID` at build time). Off until turned on, because it shows what is played
    /// to everyone who can see the Discord profile.
    pub discord_presence: bool,
    /// Last.fm scrobbling: "now playing" and a scrobble for the songs listened to. Off until turned on, because it
    /// tells a third party what is played. Only does anything when the build has a Last.fm application key and secret
    /// baked in (`PYXEE_LASTFM_API_KEY` and `PYXEE_LASTFM_API_SECRET` at build time).
    pub lastfm_scrobbling: bool,
    /// The Last.fm session key from signing in, kept so that signing in happens once. Never sent to the window. It now
    /// lives in the Windows Credential Manager; this is only where an older version left it (moved on first sight) and
    /// where it is kept on a system with no such store.
    pub lastfm_session: Option<String>,
    /// Where the mini player was last left (physical pixels).
    pub mini_bounds: Option<Rect>,
    /// The update check also offers pre-releases (release candidates). Off by default, so a stable copy is only
    /// ever moved to stable releases; a copy that already is a pre-release looks at them regardless.
    pub check_prereleases: bool,
    /// The equalizer (Settings, Sound).
    pub eq: EqSettings,
    /// The output device the listener chose (the system's own id for it). `None` follows the system default. It stays
    /// on this computer and is only ever compared with the ids of the devices that are connected.
    pub output_device: Option<String>,
}

/// The longest device id kept (the ids are short; a longer one in a damaged or edited file is not an id).
pub const OUTPUT_DEVICE_ID_MAX: usize = 512;

impl Default for ShellSettings {
    fn default() -> Self {
        Self {
            close_to_tray: false,
            start_with_windows: false,
            toast_on_change: false,
            tray_controls: false,
            flyout_on_click: true,
            discord_presence: false,
            lastfm_scrobbling: false,
            lastfm_session: None,
            mini_bounds: None,
            check_prereleases: false,
            eq: EqSettings::default(),
            output_device: None,
        }
    }
}

impl ShellSettings {
    /// Keeps stored values inside what the windows can use.
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        self.eq = self.eq.sanitized();
        self.output_device = self
            .output_device
            .filter(|id| !id.is_empty() && id.len() <= OUTPUT_DEVICE_ID_MAX);
        self.mini_bounds = self.mini_bounds.map(|r| {
            Rect::new(
                r.x.clamp(-32_000, 32_000),
                r.y.clamp(-32_000, 32_000),
                r.w.clamp(MINI_MIN.0, MINI_MAX.0),
                r.h.clamp(MINI_MIN.1, MINI_MAX.1),
            )
        });
        self
    }
}

pub struct Loaded {
    pub settings: ShellSettings,
    /// Set when the file existed but could not be used (it is kept as `shell.json.bad`).
    pub warning: Option<String>,
}

#[must_use]
pub fn load(path: &Path) -> Loaded {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Loaded {
                settings: ShellSettings::default(),
                warning: None,
            };
        }
        Err(e) => {
            return Loaded {
                settings: ShellSettings::default(),
                warning: Some(format!("could not read {}: {e}", path.display())),
            };
        }
    };
    match schema::parse::<ShellSettings>(text.as_bytes()) {
        Parsed::Usable(s) => Loaded {
            settings: s.sanitized(),
            warning: None,
        },
        Parsed::Newer(version) => {
            // Written by a newer Pyxee (someone went back to an older release): not used, and not overwritten.
            let kept = schema::set_aside(path, &format!(".v{version}"));
            Loaded {
                settings: ShellSettings::default(),
                warning: Some(format!(
                    "{} was written by a newer Pyxee (settings format {version}); using defaults{}",
                    path.display(),
                    match kept {
                        Ok(aside) => format!(", the newer file is kept as {}", aside.display()),
                        Err(_) => String::new(),
                    }
                )),
            }
        }
        Parsed::Bad(e) => {
            let bad = path.with_extension("json.bad");
            let kept = std::fs::rename(path, &bad).is_ok();
            Loaded {
                settings: ShellSettings::default(),
                warning: Some(format!(
                    "{} was damaged ({e}); using defaults{}",
                    path.display(),
                    if kept {
                        format!(", the old file is kept as {}", bad.display())
                    } else {
                        String::new()
                    }
                )),
            }
        }
    }
}

/// Saved through [`crate::atomic::write`], so a crash never leaves a half-written file.
pub fn save(path: &Path, s: &ShellSettings) -> std::io::Result<()> {
    let json = schema::to_vec_pretty(s).map_err(std::io::Error::other)?;
    crate::atomic::write(path, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("pyxee-shell-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_notification_area_defaults_are_a_flyout_on_click_and_no_extra_icons() {
        let d = ShellSettings::default();
        assert!(d.flyout_on_click && !d.tray_controls);
    }

    #[test]
    fn a_file_from_before_these_settings_existed_keeps_its_values_and_gets_the_new_defaults() {
        let p = dir("old").join("shell.json");
        std::fs::write(&p, r#"{"closeToTray": true, "toastOnChange": true}"#).unwrap();
        let l = load(&p);
        assert!(l.warning.is_none());
        assert!(l.settings.close_to_tray && l.settings.toast_on_change);
        assert!(l.settings.flyout_on_click && !l.settings.tray_controls);
        assert!(
            !l.settings.discord_presence,
            "a file that never chose Discord presence gets it off"
        );
    }

    #[test]
    fn discord_presence_is_off_by_default_and_can_be_saved_on() {
        assert!(!ShellSettings::default().discord_presence);
        let p = dir("discord").join("shell.json");
        let s = ShellSettings {
            discord_presence: true,
            ..Default::default()
        };
        save(&p, &s).unwrap();
        assert!(load(&p).settings.discord_presence);
    }

    #[test]
    fn checking_for_prereleases_is_off_by_default_also_for_an_older_file_and_can_be_saved_on() {
        assert!(!ShellSettings::default().check_prereleases);
        let p = dir("prereleases").join("shell.json");
        std::fs::write(&p, r#"{"closeToTray": true}"#).unwrap();
        assert!(!load(&p).settings.check_prereleases);
        let s = ShellSettings {
            check_prereleases: true,
            ..Default::default()
        };
        save(&p, &s).unwrap();
        assert!(load(&p).settings.check_prereleases);
    }

    #[test]
    fn a_missing_file_gives_defaults_without_a_warning() {
        let l = load(&dir("missing").join("shell.json"));
        assert_eq!(l.settings, ShellSettings::default());
        assert!(l.warning.is_none());
        assert!(
            !l.settings.close_to_tray
                && !l.settings.start_with_windows
                && !l.settings.toast_on_change
        );
    }

    #[test]
    fn settings_survive_a_save_and_load_in_a_folder_that_does_not_exist_yet() {
        let p = dir("roundtrip").join("deeper/shell.json");
        let s = ShellSettings {
            close_to_tray: true,
            toast_on_change: true,
            mini_bounds: Some(Rect::new(-1500, 300, 340, 120)),
            ..Default::default()
        };
        save(&p, &s).unwrap();
        assert_eq!(load(&p).settings, s);
        let left: Vec<_> = std::fs::read_dir(p.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(left.is_empty(), "no temporary file is left: {left:?}");
    }

    #[test]
    fn a_damaged_file_gives_defaults_a_warning_and_is_kept_aside() {
        let p = dir("damaged").join("shell.json");
        std::fs::write(&p, "{ not json").unwrap();
        let l = load(&p);
        assert_eq!(l.settings, ShellSettings::default());
        assert!(l.warning.unwrap().contains("damaged"));
        assert!(p.with_extension("json.bad").exists());
        assert!(!p.exists());
    }

    #[test]
    fn missing_and_unknown_fields_are_fine_so_older_and_newer_files_load() {
        let p = dir("compat").join("shell.json");
        std::fs::write(&p, r#"{"closeToTray":true,"somethingFromTheFuture":[1,2]}"#).unwrap();
        let l = load(&p);
        assert!(l.settings.close_to_tray && !l.settings.start_with_windows);
        assert!(l.warning.is_none());
    }

    #[test]
    fn settings_are_saved_with_their_format_version() {
        let p = dir("versioned").join("shell.json");
        let s = ShellSettings {
            close_to_tray: true,
            ..Default::default()
        };
        save(&p, &s).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(
            text.contains(&format!("\"version\": {}", schema::CURRENT)),
            "{text}"
        );
        let l = load(&p);
        assert!(l.warning.is_none());
        assert_eq!(l.settings, s);
    }

    #[test]
    fn a_file_from_a_newer_pyxee_is_kept_aside_not_used_and_not_overwritten() {
        let d = dir("newer");
        let p = d.join("shell.json");
        let text = r#"{"closeToTray":true,"version":99}"#;
        std::fs::write(&p, text).unwrap();
        let l = load(&p);
        assert_eq!(l.settings, ShellSettings::default());
        assert!(l.warning.is_some_and(|w| w.contains("newer Pyxee")));
        assert!(!p.exists());
        assert_eq!(
            std::fs::read_to_string(d.join("shell.json.v99")).unwrap(),
            text
        );
    }

    #[test]
    fn a_version_that_makes_no_sense_counts_as_a_damaged_file() {
        let p = dir("badversion").join("shell.json");
        std::fs::write(&p, r#"{"closeToTray":true,"version":"two"}"#).unwrap();
        let l = load(&p);
        assert_eq!(l.settings, ShellSettings::default());
        assert!(l.warning.is_some_and(|w| w.contains("damaged")));
        assert!(p.with_extension("json.bad").exists());
    }

    #[test]
    fn silly_window_sizes_are_clamped() {
        let s = ShellSettings {
            mini_bounds: Some(Rect::new(0, 0, 1, 100_000)),
            ..Default::default()
        }
        .sanitized();
        let b = s.mini_bounds.unwrap();
        assert_eq!((b.w, b.h), (MINI_MIN.0, MINI_MAX.1));
    }

    #[test]
    fn the_equalizer_is_off_and_flat_by_default_also_for_a_file_from_before_it_existed() {
        let d = EqSettings::default();
        assert!(!d.enabled && d.preamp == 0 && d.gains == vec![0; EQ_BANDS]);
        let p = dir("eq-old").join("shell.json");
        std::fs::write(&p, r#"{"closeToTray": true}"#).unwrap();
        assert_eq!(load(&p).settings.eq, EqSettings::default());
    }

    #[test]
    fn the_equalizer_survives_a_save_and_load() {
        let p = dir("eq-roundtrip").join("shell.json");
        let eq = EqSettings {
            enabled: true,
            preamp: -30,
            gains: vec![60, 50, 40, 20, 0, 0, -10, 0, 25, 120],
        };
        let s = ShellSettings {
            eq: eq.clone(),
            ..Default::default()
        };
        save(&p, &s).unwrap();
        assert_eq!(load(&p).settings.eq, eq);
    }

    #[test]
    fn an_equalizer_with_the_wrong_number_of_gains_or_silly_values_is_mended_not_thrown_away() {
        let p = dir("eq-mend").join("shell.json");
        std::fs::write(
            &p,
            r#"{"closeToTray":true,"eq":{"enabled":true,"preamp":5000,"gains":[1,-999,3]}}"#,
        )
        .unwrap();
        let l = load(&p);
        assert!(l.warning.is_none() && l.settings.close_to_tray);
        assert!(l.settings.eq.enabled);
        assert_eq!(l.settings.eq.preamp, EQ_LIMIT);
        assert_eq!(l.settings.eq.gains.len(), EQ_BANDS);
        assert_eq!(&l.settings.eq.gains[..4], &[1, -EQ_LIMIT, 3, 0]);
        let long = EqSettings {
            gains: vec![7; EQ_BANDS + 5],
            ..Default::default()
        }
        .sanitized();
        assert_eq!(long.gains, vec![7; EQ_BANDS]);
    }
}
