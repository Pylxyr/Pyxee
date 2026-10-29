//! Saves and restores the queue/position so playback resumes where the user left off.

use pyxee_core::Persisted;
use std::fs;
use std::path::Path;

/// Writes atomically (flushed temp file + rename) so a crash never leaves a half-written session.
#[cfg(test)]
pub fn save(path: &Path, p: &Persisted) -> std::io::Result<()> {
    let json = serde_json::to_vec(p).map_err(std::io::Error::other)?;
    pyxee_shell::atomic::write(path, &json)
}

/// Like `save`, but writes nothing when the session is exactly what `last` holds (what was written last time). The
/// 5-second autosave would otherwise rewrite and flush the same file all day while the player sits paused or idle.
/// `last` is updated only after a successful write, so a failed write is tried again next time.
pub fn save_if_changed(path: &Path, p: &Persisted, last: &mut Vec<u8>) -> std::io::Result<bool> {
    let json = serde_json::to_vec(p).map_err(std::io::Error::other)?;
    if json == *last {
        return Ok(false);
    }
    pyxee_shell::atomic::write(path, &json)?;
    *last = json;
    Ok(true)
}

/// A missing or unreadable file is not an error: the app just starts empty.
pub fn load(path: &Path) -> Option<Persisted> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyxee_core::{Command, Player, Track, Transport};

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("pyxee-session-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn an_unchanged_session_is_not_written_again() {
        let d = dir("unchanged");
        fs::create_dir_all(&d).unwrap();
        let path = d.join("session.json");
        let p = Player::new(1);
        let mut last = Vec::new();
        assert!(save_if_changed(&path, &p.persisted(), &mut last).unwrap());
        fs::remove_file(&path).unwrap();
        // Same content: nothing is written (the file stays gone).
        assert!(!save_if_changed(&path, &p.persisted(), &mut last).unwrap());
        assert!(!path.exists());
        // A change is written.
        let mut q = Player::new(1);
        q.command(Command::SetShuffle(true));
        assert!(save_if_changed(&path, &q.persisted(), &mut last).unwrap());
        assert!(path.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn round_trips_and_restores_paused_at_the_saved_position() {
        let path = dir("rt").join("session.json");
        let mut p = Player::new(1);
        p.command(Command::PlayTracks {
            tracks: vec![Track::new("a", "A", "x"), Track::new("b", "B", "y")],
            start: 1,
        });
        save(&path, &p.persisted()).unwrap();
        let restored = Player::restore(load(&path).unwrap());
        assert_eq!(restored.queue().entries().len(), 2);
        assert!(matches!(
            restored.snapshot().transport,
            Transport::Paused { loaded: false, .. }
        ));
    }

    #[test]
    fn missing_or_corrupt_file_means_a_fresh_start() {
        let d = dir("bad");
        assert!(load(&d.join("nope.json")).is_none());
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("session.json"), b"{ not json").unwrap();
        assert!(load(&d.join("session.json")).is_none());
    }

    #[test]
    fn saving_twice_replaces_the_file_without_leaving_temp_files() {
        let d = dir("twice");
        let path = d.join("session.json");
        let p = Player::new(1);
        save(&path, &p.persisted()).unwrap();
        save(&path, &p.persisted()).unwrap();
        let names: Vec<_> = fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("session.json")]);
    }
}
