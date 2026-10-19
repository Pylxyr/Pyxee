//! Where the app keeps its files, and the one-time move from the Roaming to the Local application-data folder.
//!
//! Pyxee used to keep everything in `%APPDATA%\dev.pyxee.app` (Roaming). A live SQLite file with `-wal`/`-shm`
//! side files does not belong in a folder that roaming profiles, folder redirection or backup tools sync: a
//! half-synced database is a corrupt one. `%LOCALAPPDATA%\dev.pyxee.app` never leaves the machine.
//!
//! The move is a copy, never a delete, and it never risks the user's library:
//! * nothing is overwritten: once the new folder has a `library.db`, the old folder is not looked at again;
//! * each file is copied under a temporary name and renamed, so an interrupted copy leaves no truncated database;
//! * if the library cannot be copied, the app keeps using the old folder for everything (and retries next start),
//!   instead of starting with an empty library;
//! * the originals stay where they were (a backup the user can delete).

use std::fs;
use std::path::{Path, PathBuf};

/// The database and its side files, in copy order: `library.db` last, so it only appears once its `-wal` is in place.
const DB: &str = "library.db";
const DB_FILES: [&str; 3] = ["library.db-wal", "library.db-shm", DB];
/// Small state files. Losing one costs the queue or the settings, not the library, so a failure here is only noted.
const STATE_FILES: [&str; 2] = ["session.json", "shell.json"];

/// Something worth writing to the log once logging is up (this runs before it starts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub warn: bool,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The folder to use, or `None` if the system has none to offer.
    pub dir: Option<PathBuf>,
    pub notes: Vec<Note>,
}

/// An empty file with this name next to the program turns on portable mode (the zip from the release carries one).
pub const PORTABLE_MARKER: &str = "portable.txt";
/// In portable mode everything the app writes goes in this folder next to the program.
pub const PORTABLE_DATA: &str = "data";

/// Portable mode: if `portable.txt` sits next to the program (`exe_dir`), the data folder is `exe_dir\data`, so the
/// whole app can be copied or deleted as one folder and nothing is left in the user's profile. `None`: installed mode.
/// The Roaming-to-Local move does not apply to a portable folder.
pub fn portable_dir(exe_dir: &Path) -> Option<PathBuf> {
    exe_dir
        .join(PORTABLE_MARKER)
        .is_file()
        .then(|| exe_dir.join(PORTABLE_DATA))
}

/// Picks the data folder. `old` is the former (Roaming) folder, `new` the Local one.
pub fn resolve(old: Option<&Path>, new: Option<&Path>) -> Resolved {
    let mut notes = Vec::new();
    let dir = match (old, new) {
        (old, None) => old.map(Path::to_path_buf),
        (None, Some(new)) => Some(new.to_path_buf()),
        (Some(old), Some(new)) if same(old, new) => Some(new.to_path_buf()),
        (Some(old), Some(new)) => Some(if migrate(old, new, &mut notes) {
            new.to_path_buf()
        } else {
            old.to_path_buf()
        }),
    };
    Resolved { dir, notes }
}

fn same(a: &Path, b: &Path) -> bool {
    a == b || matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

/// Copies what is in `old` into `new`. `true`: use `new`. `false`: the library could not be brought over, use `old`.
fn migrate(old: &Path, new: &Path, notes: &mut Vec<Note>) -> bool {
    // Nothing to bring over, or the new folder is already in use (its library exists): leave `old` alone.
    if !old.is_dir() || new.join(DB).exists() {
        return true;
    }
    let warn = |notes: &mut Vec<Note>, text: String| notes.push(Note { warn: true, text });
    if let Err(e) = fs::create_dir_all(new) {
        warn(
            notes,
            format!(
                "cannot create {}: {e}; keeping the library and settings in {}",
                new.display(),
                old.display()
            ),
        );
        return false;
    }
    let mut copied = Vec::new();
    if old.join(DB).is_file() {
        for name in DB_FILES {
            let src = old.join(name);
            if !src.is_file() {
                continue;
            }
            match copy_atomic(&src, &new.join(name)) {
                Ok(()) => copied.push(name),
                Err(e) => {
                    // Undo the side files so a later, different database is never opened with this one's journal.
                    for done in &copied {
                        let _ = fs::remove_file(new.join(done));
                    }
                    warn(
                        notes,
                        format!(
                            "could not copy {name} to {}: {e}; keeping the library and settings in {}",
                            new.display(),
                            old.display()
                        ),
                    );
                    return false;
                }
            }
        }
    }
    for name in STATE_FILES {
        let (src, dst) = (old.join(name), new.join(name));
        if src.is_file() && !dst.exists() {
            match copy_atomic(&src, &dst) {
                Ok(()) => copied.push(name),
                Err(e) => warn(notes, format!("could not copy {name}: {e}")),
            }
        }
    }
    if !copied.is_empty() {
        notes.push(Note {
            warn: false,
            text: format!(
                "moved {} from {} to {} (the originals were left in place and can be deleted)",
                copied.join(", "),
                old.display(),
                new.display()
            ),
        });
    }
    true
}

/// Copies `src` to `dst` through `<dst>.tmp`, checking the size, so `dst` is either complete or absent.
fn copy_atomic(src: &Path, dst: &Path) -> std::io::Result<()> {
    let mut tmp = dst.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let result = (|| {
        let n = fs::copy(src, &tmp)?;
        if n != fs::metadata(src)?.len() {
            return Err(std::io::Error::other(
                "the copy is not the size of the original",
            ));
        }
        fs::rename(&tmp, dst)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A fresh empty folder under the system temp dir, removed on drop.
    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let p = std::env::temp_dir().join(format!(
                "pyxee-datadir-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
        fn path(&self, rel: &str) -> PathBuf {
            self.0.join(rel)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(dir: &Path, name: &str, body: &str) {
        let file = dir.join(name);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, body).unwrap();
    }
    fn read(dir: &Path, name: &str) -> Option<String> {
        fs::read_to_string(dir.join(name)).ok()
    }
    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn portable_mode_needs_the_marker_next_to_the_program() {
        let t = Tmp::new();
        assert_eq!(portable_dir(&t.0), None, "no marker: installed mode");
        write(&t.0, "data/library.db", "not a marker");
        assert_eq!(
            portable_dir(&t.0),
            None,
            "a data folder alone does not turn it on"
        );
        write(&t.0, PORTABLE_MARKER, "");
        assert_eq!(portable_dir(&t.0), Some(t.0.join("data")));
        assert_eq!(
            portable_dir(&t.path("missing")),
            None,
            "a folder that does not exist is installed mode"
        );
    }

    fn old_with_everything(t: &Tmp) -> PathBuf {
        let old = t.path("roaming");
        write(&old, "library.db", "LIB");
        write(&old, "library.db-wal", "WAL");
        write(&old, "library.db-shm", "SHM");
        write(&old, "session.json", "SESSION");
        write(&old, "shell.json", "SHELL");
        write(&old, "logs/pyxee.log", "old log");
        old
    }

    #[test]
    fn the_library_and_settings_are_copied_and_the_originals_stay() {
        let t = Tmp::new();
        let old = old_with_everything(&t);
        let new = t.path("local");
        let r = resolve(Some(&old), Some(&new));
        assert_eq!(r.dir.as_deref(), Some(new.as_path()));
        for (n, body) in [
            ("library.db", "LIB"),
            ("library.db-wal", "WAL"),
            ("library.db-shm", "SHM"),
            ("session.json", "SESSION"),
            ("shell.json", "SHELL"),
        ] {
            assert_eq!(read(&new, n).as_deref(), Some(body), "{n} copied");
            assert_eq!(read(&old, n).as_deref(), Some(body), "{n} original kept");
        }
        // Logs are not worth moving; no temporary file is left behind.
        assert!(!new.join("logs").exists());
        assert!(
            names(&new).iter().all(|n| !n.ends_with(".tmp")),
            "{:?}",
            names(&new)
        );
        assert!(
            r.notes.iter().any(|n| !n.warn && n.text.contains("moved")),
            "{:?}",
            r.notes
        );
        assert!(r.notes.iter().all(|n| !n.warn));
    }

    #[test]
    fn a_library_already_in_the_new_folder_is_never_overwritten_and_the_old_one_is_not_read_again()
    {
        let t = Tmp::new();
        let old = old_with_everything(&t);
        let new = t.path("local");
        write(&new, "library.db", "NEWER");
        let r = resolve(Some(&old), Some(&new));
        assert_eq!(r.dir.as_deref(), Some(new.as_path()));
        assert_eq!(read(&new, "library.db").as_deref(), Some("NEWER"));
        assert_eq!(
            read(&new, "session.json"),
            None,
            "a stale old session must not come back"
        );
        assert!(r.notes.is_empty());
    }

    #[test]
    fn running_it_twice_changes_nothing_the_second_time() {
        let t = Tmp::new();
        let old = old_with_everything(&t);
        let new = t.path("local");
        resolve(Some(&old), Some(&new));
        write(&new, "library.db", "CHANGED SINCE");
        let again = resolve(Some(&old), Some(&new));
        assert_eq!(read(&new, "library.db").as_deref(), Some("CHANGED SINCE"));
        assert!(again.notes.is_empty());
    }

    #[test]
    fn a_fresh_install_just_uses_the_new_folder() {
        let t = Tmp::new();
        let new = t.path("local");
        let r = resolve(Some(&t.path("never-existed")), Some(&new));
        assert_eq!(r.dir.as_deref(), Some(new.as_path()));
        assert!(r.notes.is_empty());
        assert!(
            !new.exists(),
            "nothing is created until the app writes something"
        );
    }

    #[test]
    fn missing_folders_and_identical_folders_are_handled() {
        let t = Tmp::new();
        let a = t.path("a");
        assert_eq!(resolve(Some(&a), None).dir.as_deref(), Some(a.as_path()));
        assert_eq!(resolve(None, Some(&a)).dir.as_deref(), Some(a.as_path()));
        assert_eq!(resolve(None, None).dir, None);
        write(&a, "library.db", "LIB");
        let r = resolve(Some(&a), Some(&a));
        assert_eq!(r.dir.as_deref(), Some(a.as_path()));
        assert_eq!(read(&a, "library.db").as_deref(), Some("LIB"));
        assert!(r.notes.is_empty());
    }

    #[test]
    fn settings_without_a_library_are_still_moved() {
        let t = Tmp::new();
        let old = t.path("roaming");
        write(&old, "shell.json", "SHELL");
        let new = t.path("local");
        let r = resolve(Some(&old), Some(&new));
        assert_eq!(r.dir.as_deref(), Some(new.as_path()));
        assert_eq!(read(&new, "shell.json").as_deref(), Some("SHELL"));
        assert!(!new.join("library.db").exists());
    }

    #[test]
    fn if_the_new_folder_cannot_be_made_the_old_one_keeps_being_used() {
        let t = Tmp::new();
        let old = old_with_everything(&t);
        // A file where the folder should be: create_dir_all must fail.
        let new = t.path("local");
        fs::write(&new, "i am a file").unwrap();
        let r = resolve(Some(&old), Some(&new));
        assert_eq!(
            r.dir.as_deref(),
            Some(old.as_path()),
            "no data loss: stay on the old folder"
        );
        assert_eq!(read(&old, "library.db").as_deref(), Some("LIB"));
        assert!(
            r.notes
                .iter()
                .any(|n| n.warn && n.text.contains("keeping the library")),
            "{:?}",
            r.notes
        );
    }

    #[test]
    fn a_failed_library_copy_leaves_no_stray_side_files_and_keeps_the_old_folder() {
        let t = Tmp::new();
        let old = old_with_everything(&t);
        let new = t.path("local");
        // The database's temporary name is taken by a folder, so copying library.db fails
        // after its -wal and -shm were already copied.
        fs::create_dir_all(new.join("library.db.tmp")).unwrap();
        let r = resolve(Some(&old), Some(&new));
        assert_eq!(r.dir.as_deref(), Some(old.as_path()));
        assert!(!new.join("library.db").exists());
        assert!(
            !new.join("library.db-wal").exists(),
            "a journal without its database must not be left"
        );
        assert!(!new.join("library.db-shm").exists());
        assert!(
            r.notes
                .iter()
                .any(|n| n.warn && n.text.contains("library.db")),
            "{:?}",
            r.notes
        );
        // The next start can simply try again.
        fs::remove_dir_all(new.join("library.db.tmp")).unwrap();
        let retry = resolve(Some(&old), Some(&new));
        assert_eq!(retry.dir.as_deref(), Some(new.as_path()));
        assert_eq!(read(&new, "library.db").as_deref(), Some("LIB"));
    }
}
