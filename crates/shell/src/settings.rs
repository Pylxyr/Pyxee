//! Shell settings kept in `shell.json`: tolerant to a missing, damaged or newer file, written atomically.

use crate::Rect;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MINI_MIN: (i32, i32) = (260, 90);
pub const MINI_MAX: (i32, i32) = (900, 400);

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ShellSettings {
    /// The window's close button hides to the tray instead of quitting.
    pub close_to_tray: bool,
    pub start_with_windows: bool,
    /// A notification when the song changes while the window is not focused.
    pub toast_on_change: bool,
    /// Where the mini player was last left (physical pixels).
    pub mini_bounds: Option<Rect>,
}

impl ShellSettings {
    /// Keeps stored values inside what the windows can use.
    pub fn sanitized(mut self) -> Self {
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

pub fn load(path: &Path) -> Loaded {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Loaded { settings: ShellSettings::default(), warning: None };
        }
        Err(e) => {
            return Loaded {
                settings: ShellSettings::default(),
                warning: Some(format!("could not read {}: {e}", path.display())),
            };
        }
    };
    match serde_json::from_str::<ShellSettings>(&text) {
        Ok(s) => Loaded { settings: s.sanitized(), warning: None },
        Err(e) => {
            let bad = path.with_extension("json.bad");
            let kept = std::fs::rename(path, &bad).is_ok();
            Loaded {
                settings: ShellSettings::default(),
                warning: Some(format!(
                    "{} was damaged ({e}); using defaults{}",
                    path.display(),
                    if kept { format!(", the old file is kept as {}", bad.display()) } else { String::new() }
                )),
            }
        }
    }
}

/// Writes next to the target and renames over it, so a crash never leaves a half-written file.
pub fn save(path: &Path, s: &ShellSettings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(s).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
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
    fn a_missing_file_gives_defaults_without_a_warning() {
        let l = load(&dir("missing").join("shell.json"));
        assert_eq!(l.settings, ShellSettings::default());
        assert!(l.warning.is_none());
        assert!(!l.settings.close_to_tray && !l.settings.start_with_windows && !l.settings.toast_on_change);
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
        assert!(!p.with_extension("json.tmp").exists(), "no temporary file is left");
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
    fn silly_window_sizes_are_clamped() {
        let s = ShellSettings { mini_bounds: Some(Rect::new(0, 0, 1, 100_000)), ..Default::default() }.sanitized();
        let b = s.mini_bounds.unwrap();
        assert_eq!((b.w, b.h), (MINI_MIN.0, MINI_MAX.1));
    }
}
