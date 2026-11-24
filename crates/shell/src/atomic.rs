//! One way to save a small file so that a crash or a power cut never leaves half of it: the bytes go to a file next to
//! the target, are flushed to the disk, and only then take the target's place. Settings and the session both
//! save through here.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A name no other save (from another thread or another run) is using, in the target's own folder so the rename
/// below never has to cross volumes.
fn temp_name(path: &Path) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map_or_default(|n| n.to_string_lossy().into_owned());
    path.with_file_name(format!(".{name}.{}-{n}.tmp", std::process::id()))
}

/// Creates the temporary file. On unix it is made readable by its owner only (0600): these files hold the session, the
/// settings and `.env`, which have no business being world-readable. (Windows folders under the user's profile are
/// already private to the user.) The rename keeps the mode.
fn create(path: &Path) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Removes the temporary files that a crash or a power cut left in `dir` (the ones `write` names, whatever process made
/// them). Call it once at start-up, before anything saves: then none of them is in use. Returns how many were removed.
#[must_use]
pub fn sweep_stale(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_temp_name(&name)
            && entry.file_type().is_ok_and(|t| t.is_file())
            && fs::remove_file(entry.path()).is_ok()
        {
            removed += 1;
        }
    }
    removed
}

/// `.<name>.<pid>-<n>.tmp`, as `temp_name` makes it.
fn is_temp_name(name: &str) -> bool {
    let Some(inner) = name.strip_prefix('.').and_then(|n| n.strip_suffix(".tmp")) else {
        return false;
    };
    let Some((_, tail)) = inner.rsplit_once('.') else {
        return false;
    };
    let Some((pid, n)) = tail.split_once('-') else {
        return false;
    };
    !pid.is_empty()
        && !n.is_empty()
        && pid.bytes().all(|b| b.is_ascii_digit())
        && n.bytes().all(|b| b.is_ascii_digit())
}

/// Replaces `path` with `bytes` (creating its folder if needed). On any failure the old file is untouched and the
/// temporary file is removed.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    let tmp = temp_name(path);
    let saved = (|| {
        let mut file = create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()
    })()
    .and_then(|()| fs::rename(&tmp, path));
    if saved.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    saved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pyxee-atomic-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn names(d: &Path) -> Vec<String> {
        fs::read_dir(d)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_new_file_is_created_with_its_folder_and_an_old_one_is_replaced() {
        let d = dir("replace");
        let file = d.join("deeper").join("a.json");
        write(&file, b"one").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"one");
        write(&file, b"two").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"two");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn no_temporary_file_is_left_behind() {
        let d = dir("clean");
        let file = d.join("a.json");
        write(&file, b"x").unwrap();
        write(&file, b"y").unwrap();
        assert_eq!(names(&d), vec!["a.json".to_string()]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn leftover_temporary_files_are_swept_and_nothing_else() {
        let d = dir("sweep");
        fs::create_dir_all(&d).unwrap();
        for keep in [
            "session.json",
            "session.json.bad",
            ".hidden",
            ".notes.tmp",
            "a.tmp",
        ] {
            fs::write(d.join(keep), b"x").unwrap();
        }
        for stale in [".session.json.1234-0.tmp", ".settings.json.99-12.tmp"] {
            fs::write(d.join(stale), b"x").unwrap();
        }
        assert_eq!(sweep_stale(&d), 2);
        let mut left = names(&d);
        left.sort();
        assert_eq!(
            left,
            [
                ".hidden",
                ".notes.tmp",
                "a.tmp",
                "session.json",
                "session.json.bad"
            ]
        );
        assert_eq!(sweep_stale(&d.join("missing")), 0);
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_saved_file_is_readable_by_its_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let d = dir("mode");
        let file = d.join("a.json");
        write(&file, b"x").unwrap();
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_failed_save_keeps_the_old_file_and_cleans_up() {
        let d = dir("fail");
        fs::create_dir_all(&d).unwrap();
        let file = d.join("a.json");
        write(&file, b"keep").unwrap();
        // A folder where the file should go: the rename cannot succeed.
        let blocked = d.join("blocked");
        fs::create_dir_all(blocked.join("inner")).unwrap();
        assert!(write(&blocked, b"nope").is_err());
        assert_eq!(fs::read(&file).unwrap(), b"keep");
        assert!(
            names(&d).iter().all(|n| {
                !std::path::Path::new(n)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("tmp"))
            }),
            "{:?}",
            names(&d)
        );
        let _ = fs::remove_dir_all(&d);
    }
}
