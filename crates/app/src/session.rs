//! Saves and restores the queue/position so playback resumes where the user left off.

use pyxee_core::Persisted;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Writes atomically (temp file + rename) so a crash never leaves a half-written session.
pub fn save(path: &Path, p: &Persisted) -> std::io::Result<()> {
    let json = serde_json::to_vec(p).map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut f = fs::File::create(&tmp)?;
    f.write_all(&json)?;
    f.sync_all()?;
    fs::rename(&tmp, path)
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
