//! The format version of the small JSON state files (`session.json`, `shell.json`).
//!
//! Every file written through this module carries `"version": N`. A file without one was written before versions
//! existed and counts as version 1, which is what [`CURRENT`] still is: nothing has to be converted yet.
//!
//! When a release has to change a file in a way that the same file read by an older release cannot cope with (a field
//! renamed, a value that means something else), it raises [`CURRENT`] and adds a step to `MIGRATIONS` that turns a
//! version `n` document into version `n + 1`. Older files are then converted while they load, one step at a time, and
//! nobody loses their settings. Adding a field with a default needs none of this: the files are read with
//! `#[serde(default)]` and unknown fields are ignored.
//!
//! A file from a *newer* release (someone went back to an older one) is never read as if it were current and never
//! overwritten: [`parse`] reports it as [`Parsed::Newer`], and the caller moves it aside with [`set_aside`].

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The version this release writes and understands.
pub const CURRENT: u32 = 1;
/// The key that holds the version in the file.
pub const KEY: &str = "version";

/// One migration step: turns a document of version `n` into one of version `n + 1`.
type Step = fn(Value) -> Value;

/// `MIGRATIONS[n - 1]` turns version `n` into version `n + 1`, so it holds `CURRENT - 1` steps (none while `CURRENT`
/// is 1).
const MIGRATIONS: &[Step] = &[];

/// What [`upgrade`] made of a document.
#[derive(Debug, Clone, PartialEq)]
pub enum Upgraded {
    /// A document of the current version (converted, if it was older).
    Current(Value),
    /// Written by a newer release: the version it carries.
    Newer(u32),
    /// Not a JSON object, a `version` that is not a whole number from 1 up, or a missing migration step.
    Invalid,
}

/// Brings a parsed document to the current version.
#[must_use]
pub fn upgrade(doc: Value) -> Upgraded {
    upgrade_with(doc, CURRENT, MIGRATIONS)
}

fn upgrade_with(mut doc: Value, current: u32, steps: &[Step]) -> Upgraded {
    let found = match &doc {
        Value::Object(map) => match map.get(KEY) {
            None => 1,
            Some(v) => match v.as_u64().and_then(|n| u32::try_from(n).ok()) {
                Some(n) if n >= 1 => n,
                _ => return Upgraded::Invalid,
            },
        },
        _ => return Upgraded::Invalid,
    };
    if found > current {
        return Upgraded::Newer(found);
    }
    for from in found..current {
        let Some(step) = steps.get((from - 1) as usize) else {
            return Upgraded::Invalid;
        };
        doc = step(doc);
    }
    match doc {
        Value::Object(mut map) => {
            map.insert(KEY.to_owned(), Value::from(current));
            Upgraded::Current(Value::Object(map))
        }
        _ => Upgraded::Invalid,
    }
}

/// What [`parse`] made of a file's bytes.
#[derive(Debug, PartialEq)]
pub enum Parsed<T> {
    /// Read, and converted to the current version if it was older.
    Usable(T),
    /// Written by a newer release: the version it carries.
    Newer(u32),
    /// Not usable; the text says why.
    Bad(String),
}

/// Reads a state file: checks its version, converts an older one, and deserializes it.
#[must_use]
pub fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Parsed<T> {
    let doc = match serde_json::from_slice::<Value>(bytes) {
        Ok(doc) => doc,
        Err(e) => return Parsed::Bad(e.to_string()),
    };
    match upgrade(doc) {
        Upgraded::Newer(version) => Parsed::Newer(version),
        Upgraded::Invalid => {
            Parsed::Bad("the file is not a JSON object with a valid format version".to_owned())
        }
        Upgraded::Current(doc) => match serde_json::from_value(doc) {
            Ok(value) => Parsed::Usable(value),
            Err(e) => Parsed::Bad(e.to_string()),
        },
    }
}

fn with_version<T: Serialize>(value: &T) -> serde_json::Result<Value> {
    let mut doc = serde_json::to_value(value)?;
    if let Value::Object(map) = &mut doc {
        map.insert(KEY.to_owned(), Value::from(CURRENT));
    }
    Ok(doc)
}

/// Serializes `value` as compact JSON with the current version added.
pub fn to_vec<T: Serialize>(value: &T) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(&with_version(value)?)
}

/// Serializes `value` as indented JSON with the current version added.
pub fn to_vec_pretty<T: Serialize>(value: &T) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec_pretty(&with_version(value)?)
}

/// Moves a file that must not be used or overwritten to `<name><suffix>` beside it (`shell.json.v2` for a file from a
/// newer release). An earlier copy under that name is replaced. Returns where the file went.
pub fn set_aside(path: &Path, suffix: &str) -> std::io::Result<PathBuf> {
    let mut name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("the path has no file name"))?
        .to_os_string();
    name.push(suffix);
    let aside = path.with_file_name(name);
    let _ = std::fs::remove_file(&aside); // renaming over a file fails on Windows
    std::fs::rename(path, &aside)?;
    Ok(aside)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct Sample {
        name: String,
        count: u32,
    }

    fn rename_old_to_name(mut doc: Value) -> Value {
        if let Some(map) = doc.as_object_mut()
            && let Some(v) = map.remove("old")
        {
            map.insert("name".to_owned(), v);
        }
        doc
    }

    fn add_flag(mut doc: Value) -> Value {
        if let Some(map) = doc.as_object_mut() {
            map.insert("flag".to_owned(), Value::Bool(true));
        }
        doc
    }

    #[test]
    fn a_file_without_a_version_is_version_one() {
        assert_eq!(
            upgrade(json!({ "name": "x" })),
            Upgraded::Current(json!({ "name": "x", "version": 1 }))
        );
    }

    #[test]
    fn a_newer_file_is_reported_and_not_read() {
        assert_eq!(
            upgrade(json!({ "version": CURRENT + 1, "name": "x" })),
            Upgraded::Newer(CURRENT + 1)
        );
    }

    #[test]
    fn a_version_that_is_not_a_whole_number_from_one_up_is_invalid() {
        for bad in [
            json!({ "version": 0 }),
            json!({ "version": -1 }),
            json!({ "version": "2" }),
            json!({ "version": 1.5 }),
            json!({ "version": null }),
            json!([1, 2]),
            json!("text"),
        ] {
            assert_eq!(upgrade(bad.clone()), Upgraded::Invalid, "{bad}");
        }
    }

    #[test]
    fn older_files_go_through_every_step_from_their_version_on() {
        let steps: &[Step] = &[rename_old_to_name, add_flag];
        assert_eq!(
            upgrade_with(json!({ "old": "a" }), 3, steps),
            Upgraded::Current(json!({ "name": "a", "flag": true, "version": 3 }))
        );
        assert_eq!(
            upgrade_with(json!({ "version": 2, "name": "a" }), 3, steps),
            Upgraded::Current(json!({ "name": "a", "flag": true, "version": 3 }))
        );
        assert_eq!(
            upgrade_with(json!({ "version": 3, "name": "a" }), 3, steps),
            Upgraded::Current(json!({ "name": "a", "version": 3 }))
        );
    }

    #[test]
    fn a_missing_step_makes_the_file_invalid_instead_of_guessing() {
        assert_eq!(upgrade_with(json!({}), 3, &[]), Upgraded::Invalid);
    }

    #[test]
    fn what_is_written_carries_the_version_and_reads_back() {
        let sample = Sample {
            name: "a".to_owned(),
            count: 2,
        };
        for bytes in [to_vec(&sample).unwrap(), to_vec_pretty(&sample).unwrap()] {
            let doc: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(doc[KEY], CURRENT);
            assert_eq!(parse::<Sample>(&bytes), Parsed::Usable(sample_again()));
        }
    }

    fn sample_again() -> Sample {
        Sample {
            name: "a".to_owned(),
            count: 2,
        }
    }

    #[test]
    fn old_new_and_damaged_files_are_told_apart() {
        assert_eq!(
            parse::<Sample>(br#"{"name":"a","count":3}"#),
            Parsed::Usable(Sample {
                name: "a".to_owned(),
                count: 3
            })
        );
        assert_eq!(
            parse::<Sample>(br#"{"version":99,"name":"a"}"#),
            Parsed::Newer(99)
        );
        assert!(matches!(parse::<Sample>(b"not json"), Parsed::Bad(_)));
        assert!(matches!(
            parse::<Sample>(br#"{"count":"many"}"#),
            Parsed::Bad(_)
        ));
        assert!(matches!(parse::<Sample>(b"[1]"), Parsed::Bad(_)));
    }

    #[test]
    fn setting_a_file_aside_replaces_an_earlier_copy() {
        let dir = std::env::temp_dir().join(format!("pyxee-schema-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("shell.json");
        std::fs::write(&file, "new").unwrap();
        std::fs::write(dir.join("shell.json.v2"), "old").unwrap();
        let aside = set_aside(&file, ".v2").unwrap();
        assert_eq!(aside, dir.join("shell.json.v2"));
        assert!(!file.exists());
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), "new");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
