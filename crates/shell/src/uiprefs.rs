//! The page's own settings, kept in `ui-prefs.json` next to `shell.json`.
//!
//! Theme, custom themes, Look, interface size, volume, crossfade, the seek bar's style and the search filter are
//! saved by the page in the web view's `localStorage`. That store belongs to the web view's profile, not to Pyxee's
//! data folder, so it is lost when the profile is reset or removed ("Delete app data" when uninstalling), is not
//! carried along when a portable copy is moved, and is not shared between a development build and an installed one.
//! The page therefore keeps a copy of those entries here, and puts back the ones a start finds missing. Everything
//! else (the equalizer, the tray, ...) already lives in `shell.json`.
//!
//! Only entries whose name starts with [`PREFIX`] are kept, and each is bounded, so the file can neither grow without
//! limit nor hold anything but the page's own settings. Like the other state files it is versioned (see
//! [`crate::schema`]), tolerant to a missing, damaged or newer file, and written atomically.

use crate::schema::{self, Parsed};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// The file's name inside the data folder.
pub const FILE: &str = "ui-prefs.json";
/// Every entry the page keeps starts with this.
pub const PREFIX: &str = "pyxee.";
/// How many entries are kept (the page has about ten).
pub const MAX_ENTRIES: usize = 64;
/// The longest key.
pub const MAX_KEY: usize = 64;
/// The longest value (the custom themes are the biggest: thirty of them are well under this).
pub const MAX_VALUE: usize = 256 * 1024;
/// The most that is kept in total (keys and values).
pub const MAX_TOTAL: usize = 1024 * 1024;

/// The saved entries: name to text, as the page stored them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPrefs {
    pub items: BTreeMap<String, String>,
}

impl UiPrefs {
    /// Keeps the entries that are the page's own and within the limits; drops the rest, in key order, so what is kept
    /// does not depend on the order the page sent them in.
    #[must_use]
    pub fn sanitized(items: BTreeMap<String, String>) -> Self {
        let mut total = 0usize;
        let mut kept = BTreeMap::new();
        for (key, value) in items {
            let name_ok = key.len() > PREFIX.len()
                && key.len() <= MAX_KEY
                && key.starts_with(PREFIX)
                && key.bytes().all(|b| b.is_ascii_graphic());
            if !name_ok || value.len() > MAX_VALUE || kept.len() >= MAX_ENTRIES {
                continue;
            }
            if total + key.len() + value.len() > MAX_TOTAL {
                continue;
            }
            total += key.len() + value.len();
            kept.insert(key, value);
        }
        Self { items: kept }
    }
}

pub struct Loaded {
    pub prefs: UiPrefs,
    /// Set when the file existed but could not be used (it is kept beside the original).
    pub warning: Option<String>,
}

/// Reads the file. A missing one is an empty set; a file from a newer release or a damaged one is moved aside (never
/// deleted, never overwritten) and an empty set is used.
#[must_use]
pub fn load(path: &Path) -> Loaded {
    let empty = |warning| Loaded {
        prefs: UiPrefs::default(),
        warning,
    };
    let text = match std::fs::read(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return empty(None),
        Err(e) => {
            return empty(Some(format!("could not read {}: {e}", path.display())));
        }
    };
    match schema::parse::<UiPrefs>(&text) {
        Parsed::Usable(p) => Loaded {
            prefs: UiPrefs::sanitized(p.items),
            warning: None,
        },
        Parsed::Newer(version) => {
            let kept = schema::set_aside(path, &format!(".v{version}"));
            empty(Some(format!(
                "{} was written by a newer Pyxee (format {version}); not used{}",
                path.display(),
                kept.map_or_else(|_| String::new(), |a| format!(", kept as {}", a.display()))
            )))
        }
        Parsed::Bad(e) => {
            let bad = path.with_extension("json.bad");
            let kept = std::fs::rename(path, &bad).is_ok();
            empty(Some(format!(
                "{} was damaged ({e}); not used{}",
                path.display(),
                if kept {
                    format!(", the old file is kept as {}", bad.display())
                } else {
                    String::new()
                }
            )))
        }
    }
}

/// Saved through [`crate::atomic::write`], so a crash never leaves half a file.
pub fn save(path: &Path, prefs: &UiPrefs) -> std::io::Result<()> {
    let json = schema::to_vec_pretty(prefs).map_err(std::io::Error::other)?;
    crate::atomic::write(path, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("pyxee-uiprefs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn items(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn a_missing_file_is_an_empty_set_without_a_warning() {
        let l = load(&dir("missing").join(FILE));
        assert_eq!(l.prefs, UiPrefs::default());
        assert!(l.warning.is_none());
    }

    #[test]
    fn what_is_saved_is_read_back() {
        let p = dir("roundtrip").join(FILE);
        let prefs = UiPrefs::sanitized(items(&[
            ("pyxee.theme", r#"{"id":"dark"}"#),
            ("pyxee.volume", "55"),
        ]));
        save(&p, &prefs).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains(&format!("\"version\": {}", schema::CURRENT)));
        assert_eq!(load(&p).prefs, prefs);
    }

    #[test]
    fn only_the_pages_own_entries_are_kept() {
        let p = UiPrefs::sanitized(items(&[
            ("pyxee.volume", "80"),
            ("pyxee.", "empty name"),
            ("other.key", "x"),
            ("pyxee.has space", "x"),
            ("pyxee.ünï", "x"),
        ]));
        assert_eq!(p.items.keys().collect::<Vec<_>>(), ["pyxee.volume"]);
    }

    #[test]
    fn the_limits_hold() {
        let mut big = BTreeMap::new();
        big.insert("pyxee.huge".to_owned(), "x".repeat(MAX_VALUE + 1));
        big.insert("pyxee.fine".to_owned(), "x".repeat(MAX_VALUE));
        big.insert(format!("pyxee.{}", "k".repeat(MAX_KEY)), "x".to_owned());
        let p = UiPrefs::sanitized(big);
        assert_eq!(p.items.keys().collect::<Vec<_>>(), ["pyxee.fine"]);

        let many: BTreeMap<_, _> = (0..MAX_ENTRIES + 20)
            .map(|i| (format!("pyxee.k{i:03}"), "1".to_owned()))
            .collect();
        assert_eq!(UiPrefs::sanitized(many).items.len(), MAX_ENTRIES);

        let heavy: BTreeMap<_, _> = (0..8)
            .map(|i| (format!("pyxee.h{i}"), "y".repeat(MAX_VALUE)))
            .collect();
        let kept = UiPrefs::sanitized(heavy);
        let total: usize = kept.items.iter().map(|(k, v)| k.len() + v.len()).sum();
        assert!(total <= MAX_TOTAL);
        assert!(!kept.items.is_empty());
    }

    #[test]
    fn a_damaged_file_is_kept_aside_and_not_used() {
        let d = dir("bad");
        let p = d.join(FILE);
        std::fs::write(&p, "{ not json").unwrap();
        let l = load(&p);
        assert_eq!(l.prefs, UiPrefs::default());
        assert!(l.warning.unwrap().contains("damaged"));
        assert!(d.join("ui-prefs.json.bad").is_file());
        assert!(!p.exists());
    }

    #[test]
    fn a_file_from_a_newer_release_is_kept_aside_and_not_overwritten() {
        let d = dir("newer");
        let p = d.join(FILE);
        let newer = format!(
            r#"{{"version":{},"items":{{"pyxee.volume":"9"}}}}"#,
            schema::CURRENT + 1
        );
        std::fs::write(&p, &newer).unwrap();
        let l = load(&p);
        assert_eq!(l.prefs, UiPrefs::default());
        assert!(l.warning.unwrap().contains("newer"));
        let aside = d.join(format!("ui-prefs.json.v{}", schema::CURRENT + 1));
        assert_eq!(std::fs::read_to_string(aside).unwrap(), newer);
    }

    #[test]
    fn a_file_with_foreign_entries_is_cleaned_on_load() {
        let p = dir("foreign").join(FILE);
        std::fs::write(
            &p,
            r#"{"version":1,"items":{"pyxee.volume":"40","evil":"1"},"extra":true}"#,
        )
        .unwrap();
        assert_eq!(
            load(&p).prefs.items.keys().collect::<Vec<_>>(),
            ["pyxee.volume"]
        );
    }
}
