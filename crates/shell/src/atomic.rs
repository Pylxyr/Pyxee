//! One way to save a small file so that a crash or a power cut never leaves half of it: the bytes go to a file next to
//! the target, are flushed to the disk, and only then take the target's place. Settings, the session and `.env` all
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
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.{}-{n}.tmp", std::process::id()))
}

/// Replaces `path` with `bytes` (creating its folder if needed). On any failure the old file is untouched and the
/// temporary file is removed.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    let tmp = temp_name(path);
    let saved = (|| {
        let mut file = fs::File::create(&tmp)?;
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
            names(&d).iter().all(|n| !n.ends_with(".tmp")),
            "{:?}",
            names(&d)
        );
        let _ = fs::remove_dir_all(&d);
    }
}
