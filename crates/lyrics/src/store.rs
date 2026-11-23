//! Lyrics kept on disk, so a song that was looked up once shows its words again without the network (and, with the
//! offline songs, without LRCLIB being reachable at all).
//!
//! One small file per song in one folder. The name is a hash of the song's key and the key is written inside the file
//! too, so two keys that happen to share a hash never answer for each other. The oldest-used files go first when
//! there are more than [`MAX_ENTRIES`]. Nothing here decides *whether* lyrics are kept: the caller does not call
//! [`Store::put`] when the listener has not asked for songs to be kept.

use crate::Lyrics;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// How many songs' lyrics are kept (a few KiB each, so a few tens of MiB at most).
pub const MAX_ENTRIES: usize = 4000;
/// A file bigger than this is not lyrics (the longest synced lyrics are well under 100 KiB).
const MAX_FILE_BYTES: u64 = 512 * 1024;
const FORMAT: u32 = 1;
const EXTENSION: &str = "json";
const TEMP: &str = "tmp";

#[derive(Serialize)]
struct Record<'a> {
    v: u32,
    key: &'a str,
    lyrics: &'a Lyrics,
}

#[derive(Deserialize)]
struct Stored {
    v: u32,
    key: String,
    lyrics: Lyrics,
}

#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
    max_entries: usize,
}

/// The file name (without extension) of a song's lyrics: 64-bit FNV-1a of the key, which is stable between versions of
/// Rust and of this program, unlike `DefaultHasher`.
fn stem(key: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn is_entry(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == EXTENSION)
}

impl Store {
    /// A store in `dir`; the folder is created when the first lyrics are put.
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            max_entries: MAX_ENTRIES,
        }
    }

    /// The same store, keeping at most `max_entries` songs.
    #[must_use]
    pub fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = max_entries.max(1);
        self
    }

    fn path_of(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{}.{EXTENSION}", stem(key)))
    }

    /// The lyrics kept for `key`. Looking them up counts as using them. A file that cannot be read, is not in this
    /// format or belongs to another key is removed and answers nothing.
    pub fn get(&self, key: &str) -> Option<Lyrics> {
        let path = self.path_of(key);
        let meta = std::fs::metadata(&path).ok()?;
        if !meta.is_file() {
            return None;
        }
        let stored = (meta.len() <= MAX_FILE_BYTES)
            .then(|| std::fs::read(&path).ok())
            .flatten()
            .and_then(|bytes| serde_json::from_slice::<Stored>(&bytes).ok());
        match stored {
            Some(s) if s.v == FORMAT && s.key == key => {
                // The oldest-used go first when there are too many: using one makes it new again.
                if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&path) {
                    let _ = f.set_modified(SystemTime::now());
                }
                Some(s.lyrics)
            }
            Some(_) => None,
            None => {
                let _ = std::fs::remove_file(&path);
                None
            }
        }
    }

    /// Keeps `lyrics` for `key`, written under a temporary name and renamed so a file is never half there. "Nothing
    /// found" is not kept: the song may get lyrics later, and a look-up for it is cheap.
    pub fn put(&self, key: &str, lyrics: &Lyrics) -> std::io::Result<()> {
        if matches!(lyrics, Lyrics::None) {
            return Ok(());
        }
        let json = serde_json::to_vec(&Record {
            v: FORMAT,
            key,
            lyrics,
        })
        .map_err(std::io::Error::other)?;
        std::fs::create_dir_all(&self.dir)?;
        let path = self.path_of(key);
        let temp = path.with_extension(TEMP);
        std::fs::write(&temp, json)?;
        std::fs::rename(&temp, &path)?;
        self.evict();
        Ok(())
    }

    fn entries(&self) -> Vec<(PathBuf, SystemTime)> {
        let Ok(read) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        read.flatten()
            .filter_map(|e| {
                let path = e.path();
                let meta = e.metadata().ok()?;
                (meta.is_file() && is_entry(&path))
                    .then(|| (path, meta.modified().unwrap_or(SystemTime::UNIX_EPOCH)))
            })
            .collect()
    }

    /// How many songs have lyrics kept.
    pub fn len(&self) -> usize {
        self.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Deletes the oldest-used lyrics until no more than the limit are left.
    fn evict(&self) {
        let mut entries = self.entries();
        if entries.len() <= self.max_entries {
            return;
        }
        entries.sort_by_key(|e| e.1);
        let extra = entries.len() - self.max_entries;
        for (path, _) in entries.into_iter().take(extra) {
            let _ = std::fs::remove_file(path);
        }
    }

    /// Forgets all of it.
    pub fn clear(&self) {
        for (path, _) in self.entries() {
            let _ = std::fs::remove_file(path);
        }
        self.tidy();
    }

    /// A file that was being written when the app closed leaves a temporary file: nothing will ever finish it.
    pub fn tidy(&self) {
        let Ok(read) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for e in read.flatten() {
            if e.path().extension().is_some_and(|x| x == TEMP) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Line;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pyxee-lyrics-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn synced() -> Lyrics {
        Lyrics::Synced(vec![
            Line {
                time_ms: 1000,
                text: "first".into(),
            },
            Line {
                time_ms: 2500,
                text: String::new(),
            },
        ])
    }

    #[test]
    fn lyrics_come_back_as_they_were_put_and_other_songs_find_nothing() {
        let store = Store::new(scratch("roundtrip"));
        assert_eq!(store.get("yt:aaaaaaaaaaa"), None);
        store.put("yt:aaaaaaaaaaa", &synced()).unwrap();
        store
            .put("yt:bbbbbbbbbbb", &Lyrics::Plain("words".into()))
            .unwrap();
        store.put("yt:ccccccccccc", &Lyrics::Instrumental).unwrap();
        assert_eq!(store.get("yt:aaaaaaaaaaa"), Some(synced()));
        assert_eq!(
            store.get("yt:bbbbbbbbbbb"),
            Some(Lyrics::Plain("words".into()))
        );
        assert_eq!(store.get("yt:ccccccccccc"), Some(Lyrics::Instrumental));
        assert_eq!(store.get("yt:ddddddddddd"), None);
        assert_eq!(store.len(), 3);
    }

    #[test]
    fn nothing_found_is_not_kept() {
        let store = Store::new(scratch("none"));
        store.put("yt:aaaaaaaaaaa", &Lyrics::None).unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn a_file_for_another_key_or_in_another_format_answers_nothing() {
        let dir = scratch("foreign");
        let store = Store::new(dir.clone());
        store.put("yt:aaaaaaaaaaa", &synced()).unwrap();
        // The same file name asked for under a different key (a hash collision) is not an answer.
        std::fs::rename(
            store.path_of("yt:aaaaaaaaaaa"),
            store.path_of("yt:bbbbbbbbbbb"),
        )
        .unwrap();
        assert_eq!(store.get("yt:bbbbbbbbbbb"), None);
        // Garbage is removed rather than kept around.
        std::fs::write(store.path_of("yt:ccccccccccc"), b"not json").unwrap();
        assert_eq!(store.get("yt:ccccccccccc"), None);
        assert!(!store.path_of("yt:ccccccccccc").exists());
    }

    #[test]
    fn the_oldest_used_lyrics_go_first_when_there_are_too_many() {
        let store = Store::new(scratch("evict")).with_max_entries(2);
        store.put("yt:aaaaaaaaaaa", &synced()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        store.put("yt:bbbbbbbbbbb", &synced()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        // Using the first one makes it the newest.
        assert!(store.get("yt:aaaaaaaaaaa").is_some());
        std::thread::sleep(std::time::Duration::from_millis(20));
        store.put("yt:ccccccccccc", &synced()).unwrap();
        assert_eq!(store.len(), 2);
        assert!(store.get("yt:aaaaaaaaaaa").is_some());
        assert!(store.get("yt:bbbbbbbbbbb").is_none());
        assert!(store.get("yt:ccccccccccc").is_some());
    }

    #[test]
    fn clear_forgets_everything_including_a_half_written_file() {
        let dir = scratch("clear");
        let store = Store::new(dir.clone());
        store.put("yt:aaaaaaaaaaa", &synced()).unwrap();
        std::fs::write(dir.join("0000000000000000.tmp"), b"{").unwrap();
        store.clear();
        assert!(store.is_empty());
        assert!(!dir.join("0000000000000000.tmp").exists());
    }

    #[test]
    fn a_name_is_stable_and_differs_between_keys() {
        assert_eq!(stem("yt:aaaaaaaaaaa"), stem("yt:aaaaaaaaaaa"));
        assert_ne!(stem("yt:aaaaaaaaaaa"), stem("yt:aaaaaaaaaab"));
        assert_eq!(stem("").len(), 16);
    }
}
