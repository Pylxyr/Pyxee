//! The songs on this PC: the folders the listener chose, what a scan of them found, and the commands the window uses.
//!
//! The folders and the scan result are kept in `local-library.json` in the data folder, so the list is there at once
//! when the app starts and a scan only has to read the files that changed (see `pyxee_sources::local::scan`).

// Tauri hands a command its arguments by value (`State`, `AppHandle`, the deserialised payload): the signature is not ours to change.
#![allow(clippy::needless_pass_by_value)]

use crate::dto::{LocalLibraryDto, TrackDto};
use pyxee_sources::local::{self, LocalTrack};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

pub const LOCAL_EVENT: &str = "local-changed";

/// A list longer than this is cut: more rows than anyone scrolls, and the window has to draw them.
const MAX_LISTED: usize = 10_000;
/// The most files looked at for one `local_missing` question (a list on screen is far shorter).
const MAX_CHECKED: usize = 2_000;

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    folders: Vec<String>,
    #[serde(default)]
    tracks: Vec<LocalTrack>,
}

#[derive(Default)]
struct Inner {
    folders: Vec<PathBuf>,
    tracks: Vec<LocalTrack>,
    /// Chosen folders the last scan could not list (a drive that is not connected, a folder that was renamed). Their
    /// songs stay in `tracks`; the window says why they cannot be played.
    unreachable: Vec<PathBuf>,
    scanning: bool,
    /// Increases with every change, so the window can tell an old push from a new one.
    revision: u32,
}

#[derive(Default)]
pub struct LocalLibrary {
    path: Option<PathBuf>,
    inner: Mutex<Inner>,
    /// Changes whenever the folders do: a scan that began under an older number is stopped and its result dropped.
    generation: AtomicU64,
}

impl LocalLibrary {
    /// What `path` holds, or an empty library when there is no file, it cannot be read, or it is not ours.
    pub fn load(path: Option<PathBuf>) -> Self {
        let saved = path
            .as_deref()
            .and_then(|p| match std::fs::read(p) {
                Ok(bytes) => serde_json::from_slice::<Saved>(&bytes)
                    .map_err(|e| {
                        log::warn!(
                            "the local library file could not be read ({e}); starting empty"
                        );
                    })
                    .ok(),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => {
                    log::warn!("could not read the local library file: {e}");
                    None
                }
            })
            .unwrap_or_default();
        Self {
            path,
            inner: Mutex::new(Inner {
                folders: saved.folders.into_iter().map(PathBuf::from).collect(),
                tracks: saved.tracks,
                unreachable: Vec::new(),
                scanning: false,
                revision: 1,
            }),
            generation: AtomicU64::new(0),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn snapshot_of(inner: &Inner) -> LocalLibraryDto {
        LocalLibraryDto {
            revision: inner.revision,
            folders: inner
                .folders
                .iter()
                .map(|f| f.display().to_string())
                .collect(),
            track_count: u32::try_from(inner.tracks.len()).unwrap_or(u32::MAX),
            scanning: inner.scanning,
            unreachable: inner
                .unreachable
                .iter()
                .map(|f| f.display().to_string())
                .collect(),
        }
    }

    pub fn snapshot(&self) -> LocalLibraryDto {
        Self::snapshot_of(&self.lock())
    }

    pub fn has_folders(&self) -> bool {
        !self.lock().folders.is_empty()
    }

    /// The songs whose title, artist or album contain `query` (all of them for an empty one), in collection order.
    pub fn tracks(&self, query: &str) -> Vec<TrackDto> {
        let needle = query.trim().to_lowercase();
        self.lock()
            .tracks
            .iter()
            .filter(|t| t.matches(&needle))
            .take(MAX_LISTED)
            .map(|t| TrackDto::from(&t.to_info()))
            .collect()
    }

    /// Writes the folders and the songs. Called without the lock held: the disk can be slow.
    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let saved = {
            let g = self.lock();
            Saved {
                folders: g.folders.iter().map(|f| f.display().to_string()).collect(),
                tracks: g.tracks.clone(),
            }
        };
        let result = serde_json::to_vec(&saved)
            .map_err(std::io::Error::other)
            .and_then(|json| pyxee_shell::atomic::write(path, &json));
        if let Err(e) = result {
            log::warn!("could not save the local library: {e}");
        }
    }

    /// Adds `folder`, unless it is already covered by one in the list; folders it covers are dropped from the list.
    /// `true` when the list changed.
    fn add_folder(&self, folder: &Path) -> bool {
        let mut g = self.lock();
        if g.folders.iter().any(|f| folder.starts_with(f)) {
            return false;
        }
        g.folders.retain(|f| !f.starts_with(folder));
        g.folders.push(folder.to_path_buf());
        g.revision += 1;
        true
    }

    /// Takes `folder` out of the list, with the songs that only it held, and stops a scan that is running.
    fn remove_folder(&self, folder: &str) -> bool {
        let mut g = self.lock();
        let before = g.folders.len();
        g.folders.retain(|f| f.display().to_string() != folder);
        if g.folders.len() == before {
            return false;
        }
        let Inner {
            folders,
            tracks,
            unreachable,
            ..
        } = &mut *g;
        tracks.retain(|t| folders.iter().any(|f| Path::new(&t.path).starts_with(f)));
        unreachable.retain(|u| folders.contains(u));
        // The scan that was running covered the folder just removed; the songs it had found of the others are kept.
        self.generation.fetch_add(1, Ordering::SeqCst);
        g.scanning = false;
        g.revision += 1;
        true
    }
}

fn emit(app: &AppHandle) {
    let snapshot = app.state::<LocalLibrary>().snapshot();
    if let Err(e) = crate::shell::emit_awake(app, LOCAL_EVENT, snapshot) {
        log::warn!("could not emit the local library: {e}");
    }
}

fn notify(app: &AppHandle, message: String) {
    log::info!("notice shown to the listener: {message}");
    if let Err(e) = crate::shell::emit_noted(
        app,
        crate::state::NOTICE_EVENT,
        crate::dto::NoticeDto { message },
    ) {
        log::warn!("could not send a notice to the window: {e}");
    }
}

/// What to tell the listener after a scan: songs that are gone from their folders, and folders that could not be
/// reached (whose songs were kept). Nothing when the scan found nothing to say.
fn scan_notices(removed: usize, unreachable: &[PathBuf]) -> Vec<String> {
    let mut notices = Vec::new();
    if removed > 0 {
        let (songs, are) = if removed == 1 {
            ("1 song is".to_string(), "it was")
        } else {
            (format!("{removed} songs are"), "they were")
        };
        notices.push(format!(
            "{songs} no longer in your music folders (deleted or moved), so {are} taken out of the list. \
             Playlists, likes and the queue still hold them, marked as missing."
        ));
    }
    if !unreachable.is_empty() {
        let names: Vec<String> = unreachable
            .iter()
            .map(|f| f.display().to_string())
            .collect();
        let (what, its) = if names.len() == 1 {
            ("This folder can't be reached", "its songs stay")
        } else {
            ("These folders can't be reached", "their songs stay")
        };
        notices.push(format!(
            "{what}: {}. Is the drive or network location connected? Until it is, {its} in your list but can't be played.",
            names.join(", ")
        ));
    }
    notices
}

/// Which of these songs' files are gone (moved, renamed, deleted, or on a drive that is not connected). Only `file:`
/// keys are looked at, and only a file that is plainly not there counts: one that cannot be told (no permission) is
/// left alone, so a song is never marked missing on a guess.
fn missing_keys(keys: &[String]) -> Vec<String> {
    keys.iter()
        .take(MAX_CHECKED)
        .filter(|key| match pyxee_sources::key::parse(key) {
            Some(pyxee_sources::key::TrackKey::File(path)) => {
                Path::new(path).is_absolute()
                    && matches!(
                        std::fs::metadata(path),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound
                    )
            }
            _ => false,
        })
        .cloned()
        .collect()
}

/// Reads the chosen folders again on a thread of its own, and replaces the list of songs with what it finds. A scan
/// that is already running is stopped (its folders may have changed). Only the files that changed are read.
pub fn rescan(app: &AppHandle) {
    let lib = app.state::<LocalLibrary>();
    let (generation, folders, previous) = {
        let mut g = lib.lock();
        g.scanning = true;
        g.revision += 1;
        (
            lib.generation.fetch_add(1, Ordering::SeqCst) + 1,
            g.folders.clone(),
            g.tracks.clone(),
        )
    };
    emit(app);
    let handle = app.clone();
    let spawned = std::thread::Builder::new()
        .name("local-scan".into())
        .spawn(move || {
            let lib = handle.state::<LocalLibrary>();
            let started = Instant::now();
            let found = local::scan_folders(&folders, &previous, &|| {
                lib.generation.load(Ordering::SeqCst) != generation
            });
            // `None`: the folders changed meanwhile, and the scan that change started reports.
            let Some(scan) = found else {
                return;
            };
            let notices = scan_notices(scan.removed, &scan.unreachable);
            let local::Scan {
                tracks,
                unreachable,
                ..
            } = scan;
            {
                let mut g = lib.lock();
                if lib.generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                log::info!(
                    "local library: {} songs in {} folder(s), scanned in {:.1}s",
                    tracks.len(),
                    folders.len(),
                    started.elapsed().as_secs_f32()
                );
                g.tracks = tracks;
                g.unreachable = unreachable;
                g.scanning = false;
                g.revision += 1;
            }
            lib.save();
            emit(&handle);
            for message in notices {
                notify(&handle, message);
            }
        });
    if let Err(e) = spawned {
        log::warn!("could not start the local scan: {e}");
        let lib = app.state::<LocalLibrary>();
        lib.lock().scanning = false;
        emit(app);
    }
}

#[tauri::command]
pub fn local_get(state: State<LocalLibrary>) -> LocalLibraryDto {
    state.snapshot()
}

#[tauri::command(async)]
pub fn local_tracks(state: State<LocalLibrary>, query: String) -> Vec<TrackDto> {
    state.tracks(&query)
}

/// Asks which folder to add, in the system's own dialog (on a thread of its own: the dialog waits for the answer), and
/// scans it. Resolves to the library as it is now; choosing nothing changes nothing.
#[tauri::command]
pub async fn local_add_folder(app: AppHandle) -> Result<LocalLibraryDto, String> {
    let asker = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        asker
            .dialog()
            .file()
            .set_title("Choose a folder with music")
            .blocking_pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    let lib = app.state::<LocalLibrary>();
    let Some(picked) = picked else {
        return Ok(lib.snapshot());
    };
    let folder = picked.into_path().map_err(|e| e.to_string())?;
    // Only a folder on this PC: a network path would be walked with every song of it read over the wire.
    if !folder.is_dir() || pyxee_sources::key::parse(&pyxee_sources::key::file(&folder)).is_none() {
        return Err("Pick a folder on this PC.".into());
    }
    if lib.add_folder(&folder) {
        lib.save();
        rescan(&app);
    }
    Ok(lib.snapshot())
}

#[tauri::command]
pub fn local_remove_folder(app: AppHandle, folder: String) -> LocalLibraryDto {
    let lib = app.state::<LocalLibrary>();
    if lib.remove_folder(&folder) {
        lib.save();
        emit(&app);
    }
    lib.snapshot()
}

/// Which of these songs (keys) have lost their file, so the lists can mark them. Looks at the disk, so it runs off the
/// window's thread.
#[tauri::command]
pub async fn local_missing(keys: Vec<String>) -> Vec<String> {
    tauri::async_runtime::spawn_blocking(move || missing_keys(&keys))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub fn local_rescan(app: AppHandle) -> LocalLibraryDto {
    rescan(&app);
    app.state::<LocalLibrary>().snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(path: &str, title: &str) -> LocalTrack {
        LocalTrack {
            path: path.into(),
            title: title.into(),
            artist: "Band".into(),
            album: String::new(),
            duration_ms: None,
            modified: 1,
            size: 1,
        }
    }

    #[test]
    fn a_scan_that_lost_songs_or_folders_says_so_and_a_quiet_one_says_nothing() {
        assert_eq!(scan_notices(0, &[]), [] as [std::string::String; 0]);
        let one = scan_notices(1, &[]);
        assert_eq!(one.len(), 1);
        assert!(one[0].starts_with("1 song is no longer"), "{one:?}");
        assert!(one[0].contains("it was taken out"), "{one:?}");
        let many = scan_notices(3, &[PathBuf::from("E:\\Music"), PathBuf::from("F:\\More")]);
        assert_eq!(many.len(), 2);
        assert!(many[0].starts_with("3 songs are no longer"), "{many:?}");
        assert!(many[0].contains("they were taken out"), "{many:?}");
        assert!(
            many[1].contains("E:\\Music, F:\\More") && many[1].contains("their songs stay"),
            "{many:?}"
        );
        let single = scan_notices(0, &[PathBuf::from("E:\\Music")]);
        assert!(
            single[0].starts_with("This folder can't be reached"),
            "{single:?}"
        );
    }

    #[test]
    fn only_a_file_that_is_plainly_not_there_is_reported_missing() {
        let dir = std::env::temp_dir().join(format!("pyxee-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let there = dir.join("there.mp3");
        std::fs::write(&there, b"x").unwrap();
        let gone = dir.join("gone.mp3");
        let keys = vec![
            pyxee_sources::key::file(&there),
            pyxee_sources::key::file(&gone),
            "yt:abcdefghijk".to_string(),
            "file:relative/song.mp3".to_string(),
        ];
        assert_eq!(missing_keys(&keys), vec![pyxee_sources::key::file(&gone)]);
    }

    fn with(folders: &[&str], tracks: Vec<LocalTrack>) -> LocalLibrary {
        let lib = LocalLibrary::default();
        {
            let mut g = lib.lock();
            g.folders = folders.iter().map(PathBuf::from).collect();
            g.tracks = tracks;
        }
        lib
    }

    #[test]
    fn a_folder_inside_a_listed_one_is_not_added_and_one_that_holds_listed_ones_replaces_them() {
        let lib = with(&["/music/rock"], Vec::new());
        assert!(!lib.add_folder(Path::new("/music/rock")), "the same folder");
        assert!(
            !lib.add_folder(Path::new("/music/rock/sub")),
            "inside a listed one"
        );
        assert!(lib.add_folder(Path::new("/other")));
        assert!(lib.add_folder(Path::new("/music")));
        assert_eq!(
            lib.snapshot().folders.len(),
            2,
            "/music took the place of /music/rock"
        );
        assert!(lib.snapshot().folders.iter().all(|f| f != "/music/rock"));
    }

    #[test]
    fn removing_a_folder_drops_the_songs_only_it_held_and_stops_a_scan() {
        let lib = with(
            &["/a", "/b"],
            vec![song("/a/1.mp3", "One"), song("/b/2.mp3", "Two")],
        );
        lib.lock().scanning = true;
        let before = lib.generation.load(Ordering::SeqCst);
        assert!(!lib.remove_folder("/nowhere"));
        assert!(lib.remove_folder("/a"));
        let titles: Vec<String> = lib.tracks("").into_iter().map(|t| t.title).collect();
        assert_eq!(titles, ["Two"]);
        let s = lib.snapshot();
        assert_eq!(
            (s.folders, s.track_count, s.scanning),
            (vec!["/b".to_string()], 1, false)
        );
        assert_ne!(
            lib.generation.load(Ordering::SeqCst),
            before,
            "the running scan is told to stop"
        );
    }

    #[test]
    fn songs_are_listed_by_query_and_every_change_raises_the_revision() {
        let lib = with(
            &["/a"],
            vec![song("/a/1.mp3", "Blue"), song("/a/2.mp3", "Green")],
        );
        assert_eq!(lib.tracks("  GRE ").len(), 1);
        assert_eq!(lib.tracks("band").len(), 2, "the artist matches too");
        assert_eq!(lib.tracks("zzz"), [] as [crate::dto::TrackDto; 0]);
        assert_eq!(lib.tracks("")[0].key, "file:/a/1.mp3");
        let before = lib.snapshot().revision;
        lib.add_folder(Path::new("/c"));
        assert!(lib.snapshot().revision > before);
    }

    #[test]
    fn the_library_survives_a_restart_and_a_bad_file_starts_it_empty() {
        let dir = std::env::temp_dir().join(format!("pyxee-locallib-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("local-library.json");
        let lib = LocalLibrary::load(Some(file.clone()));
        assert_eq!(lib.snapshot().track_count, 0, "no file yet");
        {
            let mut g = lib.lock();
            g.folders = vec![PathBuf::from("/a")];
            g.tracks = vec![song("/a/1.mp3", "One")];
        }
        lib.save();
        let again = LocalLibrary::load(Some(file.clone()));
        assert_eq!(again.snapshot().folders, vec!["/a".to_string()]);
        assert_eq!(again.tracks("")[0].title, "One");
        assert!(again.has_folders());

        std::fs::write(&file, b"{ not json").unwrap();
        assert!(!LocalLibrary::load(Some(file)).has_folders());
    }
}
