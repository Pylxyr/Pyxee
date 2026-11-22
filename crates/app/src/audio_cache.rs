//! Songs kept on disk after they were played, so they play again without the network, without resolving their address
//! and without yt-dlp: instantly, and when YouTube does not answer.
//!
//! A song is kept when its stream was downloaded completely while it played (the range reader reports that, see
//! `pyxee_audio::http::CompletionHook`). The files live in one folder, named after the video, and the oldest-used ones
//! are deleted when the folder grows past the limit the listener chose.

// Tauri hands a command its arguments by value (`State`, the deserialised payload): the signature is not ours to change.
#![allow(clippy::needless_pass_by_value)]

use crate::dto::AudioCacheDto;
use pyxee_audio::http::CompletionHook;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use tauri::State;

/// What a song is kept as: what YouTube serves for it, which is also all the player has to read.
const EXTENSIONS: [&str; 2] = ["webm", "m4a"];
const PART: &str = "part";
const MIB: u64 = 1024 * 1024;
/// The limit until the listener chooses one.
pub const DEFAULT_LIMIT_MB: u32 = 1024;

#[derive(Serialize, Deserialize)]
struct Saved {
    limit_mb: u32,
}

pub struct AudioCache {
    /// `None`: there is no data folder to keep songs in, so nothing is kept.
    dir: Option<PathBuf>,
    /// Where the limit is kept between runs.
    settings: Option<PathBuf>,
    limit_bytes: AtomicU64,
}

/// What the Offline page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub songs: u32,
    pub used_mb: u32,
    pub limit_mb: u32,
}

/// The video id of a `yt:` key, when it is one that is safe to use as a file name.
fn stem(key: &str) -> Option<&str> {
    let id = key.strip_prefix("yt:")?;
    (id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
    .then_some(id)
}

fn extension_of(ext: &str) -> Option<&'static str> {
    EXTENSIONS
        .iter()
        .copied()
        .find(|e| e.eq_ignore_ascii_case(ext))
}

impl AudioCache {
    /// The cache in `dir` (created when the first song is kept), with the limit saved in `settings` or the default.
    pub fn open(dir: Option<PathBuf>, settings: Option<PathBuf>) -> Self {
        let limit_mb = settings
            .as_deref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Saved>(&b).ok())
            .map_or(DEFAULT_LIMIT_MB, |s| s.limit_mb);
        let cache = Self {
            dir,
            settings,
            limit_bytes: AtomicU64::new(u64::from(limit_mb) * MIB),
        };
        cache.remove_partial_files();
        cache
    }

    fn limit_mb(&self) -> u32 {
        u32::try_from(self.limit_bytes.load(Ordering::Relaxed) / MIB).unwrap_or(u32::MAX)
    }

    /// Whether songs are kept at all.
    pub fn enabled(&self) -> bool {
        self.dir.is_some() && self.limit_bytes.load(Ordering::Relaxed) > 0
    }

    /// Changes the limit (0 keeps nothing and empties the folder), remembers it, and deletes what no longer fits.
    pub fn set_limit_mb(&self, mb: u32) {
        self.limit_bytes
            .store(u64::from(mb) * MIB, Ordering::Relaxed);
        if let Some(p) = &self.settings {
            let saved = serde_json::to_vec(&Saved { limit_mb: mb });
            if let Err(e) = saved
                .map_err(std::io::Error::other)
                .and_then(|json| pyxee_shell::atomic::write(p, &json))
            {
                log::warn!("could not save the offline cache limit: {e}");
            }
        }
        if mb == 0 {
            self.clear();
        } else {
            self.evict();
        }
    }

    fn files(&self) -> Vec<(PathBuf, u64, SystemTime)> {
        let Some(dir) = &self.dir else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                let ext = path.extension()?.to_str()?;
                extension_of(ext)?;
                let meta = e.metadata().ok()?;
                meta.is_file().then(|| {
                    (
                        path,
                        meta.len(),
                        meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    )
                })
            })
            .collect()
    }

    pub fn usage(&self) -> Usage {
        let files = self.files();
        let bytes: u64 = files.iter().map(|f| f.1).sum();
        Usage {
            songs: u32::try_from(files.len()).unwrap_or(u32::MAX),
            used_mb: u32::try_from(bytes.div_ceil(MIB)).unwrap_or(u32::MAX),
            limit_mb: self.limit_mb(),
        }
    }

    /// The kept file of the song `key` and its extension, if there is one. Looking it up counts as using it.
    pub fn lookup(&self, key: &str) -> Option<(PathBuf, &'static str)> {
        if !self.enabled() {
            return None;
        }
        let id = stem(key)?;
        let dir = self.dir.as_ref()?;
        for ext in EXTENSIONS {
            let path = dir.join(format!("{id}.{ext}"));
            if std::fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() > 0) {
                // The oldest-used go first when space is needed: opening one makes it new again.
                if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&path) {
                    let _ = f.set_modified(SystemTime::now());
                }
                return Some((path, ext));
            }
        }
        None
    }

    /// Forgets the kept file of `key` (it could not be played).
    pub fn remove(&self, key: &str) {
        let (Some(id), Some(dir)) = (stem(key), &self.dir) else {
            return;
        };
        for ext in EXTENSIONS {
            let _ = std::fs::remove_file(dir.join(format!("{id}.{ext}")));
        }
    }

    /// Keeps the song `key`, whose stream was `ext`, as the chunks put together. Written under a temporary name and
    /// renamed, so a song is never half there.
    pub fn store(&self, key: &str, ext: &str, chunks: &[Arc<Vec<u8>>]) {
        use std::io::Write;
        if !self.enabled() {
            return;
        }
        let (Some(id), Some(ext), Some(dir)) = (stem(key), extension_of(ext), &self.dir) else {
            return;
        };
        let size: u64 = chunks.iter().map(|c| c.len() as u64).sum();
        if size == 0 || size > self.limit_bytes.load(Ordering::Relaxed) {
            return;
        }
        let write = || -> std::io::Result<()> {
            std::fs::create_dir_all(dir)?;
            let part = dir.join(format!("{id}.{ext}.{PART}"));
            let mut file = std::fs::File::create(&part)?;
            for c in chunks {
                file.write_all(c)?;
            }
            drop(file);
            std::fs::rename(&part, dir.join(format!("{id}.{ext}")))
        };
        match write() {
            Ok(()) => {
                log::info!("{key}: kept for offline replay ({} KiB)", size / 1024);
                self.evict();
            }
            Err(e) => log::warn!("could not keep {key} for offline replay: {e}"),
        }
    }

    /// What the range reader calls when a song of `key` (a stream of type `ext`) has been downloaded completely. `None`
    /// when the song cannot or should not be kept.
    pub fn hook_for(self: &Arc<Self>, key: &str, ext: &str) -> Option<CompletionHook> {
        if !self.enabled() || stem(key).is_none() || extension_of(ext).is_none() {
            return None;
        }
        let (cache, key, ext) = (Arc::clone(self), key.to_string(), ext.to_string());
        Some(Arc::new(move |chunks: &[Arc<Vec<u8>>]| {
            cache.store(&key, &ext, chunks);
        }))
    }

    /// Deletes the oldest-used songs until what is kept fits the limit.
    fn evict(&self) {
        let limit = self.limit_bytes.load(Ordering::Relaxed);
        let mut files = self.files();
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        files.sort_by_key(|f| f.2);
        for (path, len, _) in files {
            if total <= limit {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(len);
            }
        }
    }

    pub fn clear(&self) {
        for (path, _, _) in self.files() {
            let _ = std::fs::remove_file(path);
        }
    }

    /// A song that was being written when the app closed leaves a `.part` file: nothing will ever finish it.
    fn remove_partial_files(&self) {
        let Some(dir) = &self.dir else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            if e.path().extension().is_some_and(|x| x == PART) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

impl From<Usage> for AudioCacheDto {
    fn from(u: Usage) -> Self {
        Self {
            songs: u.songs,
            used_mb: u.used_mb,
            limit_mb: u.limit_mb,
        }
    }
}

#[tauri::command(async)]
pub fn audio_cache_get(cache: State<Arc<AudioCache>>) -> AudioCacheDto {
    cache.usage().into()
}

/// 0 turns the cache off and empties it.
#[tauri::command(async)]
pub fn audio_cache_set_limit(cache: State<Arc<AudioCache>>, limit_mb: u32) -> AudioCacheDto {
    cache.set_limit_mb(limit_mb);
    cache.usage().into()
}

#[tauri::command(async)]
pub fn audio_cache_clear(cache: State<Arc<AudioCache>>) -> AudioCacheDto {
    cache.clear();
    cache.usage().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "yt:aaaaaaaaaaa";
    const B: &str = "yt:bbbbbbbbbbb";
    const C: &str = "yt:ccccccccccc";

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pyxee-audiocache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn chunks(bytes: usize) -> Vec<Arc<Vec<u8>>> {
        vec![
            Arc::new(vec![7u8; bytes / 2]),
            Arc::new(vec![8u8; bytes - bytes / 2]),
        ]
    }

    fn cache(dir: &std::path::Path, limit_mb: u32) -> Arc<AudioCache> {
        let c = Arc::new(AudioCache::open(Some(dir.to_path_buf()), None));
        c.limit_bytes
            .store(u64::from(limit_mb) * MIB, Ordering::Relaxed);
        c
    }

    #[test]
    fn a_stored_song_is_found_again_with_every_byte_and_unknown_songs_are_not() {
        let dir = scratch("roundtrip");
        let c = cache(&dir, 100);
        assert!(c.lookup(A).is_none());
        c.store(A, "webm", &chunks(3000));
        let (path, ext) = c.lookup(A).expect("kept");
        assert_eq!(ext, "webm");
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), 3000);
        assert!(bytes[..1500].iter().all(|b| *b == 7) && bytes[1500..].iter().all(|b| *b == 8));
        assert!(c.lookup(B).is_none());
        c.remove(A);
        assert!(c.lookup(A).is_none());
    }

    #[test]
    fn only_youtube_songs_of_a_playable_kind_are_kept_and_names_cannot_escape_the_folder() {
        let dir = scratch("names");
        let c = cache(&dir, 100);
        c.store("file:/music/x.mp3", "webm", &chunks(100));
        c.store("yt:../../etc/passwd", "webm", &chunks(100));
        c.store("yt:short", "webm", &chunks(100));
        c.store(A, "exe", &chunks(100));
        c.store(A, "mp3", &chunks(100));
        assert_eq!(c.usage().songs, 0);
        assert!(c.hook_for("file:/x", "webm").is_none());
        assert!(c.hook_for(A, "exe").is_none());
        assert!(c.hook_for(A, "m4a").is_some());
    }

    #[test]
    fn the_oldest_used_songs_go_first_when_the_limit_is_passed() {
        let dir = scratch("evict");
        let c = cache(&dir, 3);
        let mib = usize::try_from(MIB).unwrap();
        c.store(A, "webm", &chunks(mib + mib / 2));
        std::thread::sleep(std::time::Duration::from_millis(30));
        c.store(B, "webm", &chunks(mib + mib / 2));
        std::thread::sleep(std::time::Duration::from_millis(30));
        // Playing A again makes it the newest, so B is the oldest when C needs the room.
        assert!(c.lookup(A).is_some());
        std::thread::sleep(std::time::Duration::from_millis(30));
        c.store(C, "webm", &chunks(mib + mib / 2));
        assert!(c.lookup(A).is_some(), "recently used: kept");
        assert!(c.lookup(B).is_none(), "oldest: deleted");
        assert!(c.lookup(C).is_some());
        assert!(c.usage().used_mb <= 3);
    }

    #[test]
    fn a_song_bigger_than_the_whole_limit_is_not_kept_and_zero_keeps_nothing() {
        let dir = scratch("limits");
        let c = cache(&dir, 1);
        let mib = usize::try_from(MIB).unwrap();
        c.store(A, "webm", &chunks(2 * mib));
        assert_eq!(c.usage().songs, 0);
        c.set_limit_mb(50);
        c.store(A, "webm", &chunks(1000));
        assert_eq!(c.usage().songs, 1);
        c.set_limit_mb(0);
        assert_eq!(c.usage().songs, 0, "turning it off empties it");
        assert!(!c.enabled());
        c.store(B, "webm", &chunks(1000));
        assert!(c.lookup(B).is_none());
    }

    #[test]
    fn the_limit_survives_a_restart_and_half_written_files_are_swept() {
        let dir = scratch("restart");
        std::fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("settings.json");
        let c = AudioCache::open(Some(dir.clone()), Some(settings.clone()));
        assert_eq!(c.usage().limit_mb, DEFAULT_LIMIT_MB);
        c.set_limit_mb(256);
        std::fs::write(dir.join("aaaaaaaaaaa.webm.part"), b"half").unwrap();
        let again = AudioCache::open(Some(dir.clone()), Some(settings));
        assert_eq!(again.usage().limit_mb, 256);
        assert!(!dir.join("aaaaaaaaaaa.webm.part").exists());
    }

    #[test]
    fn without_a_folder_nothing_is_kept_and_nothing_breaks() {
        let c = Arc::new(AudioCache::open(None, None));
        assert!(!c.enabled());
        c.store(A, "webm", &chunks(100));
        assert!(c.lookup(A).is_none());
        assert!(c.hook_for(A, "webm").is_none());
        assert_eq!(c.usage().songs, 0);
        c.clear();
    }
}
