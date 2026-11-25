//! Playlists as files: "Export" writes one playlist to a `.json` file the listener picks, "Import" reads one back and
//! makes it a new playlist. Both ask for the file in the system's own dialog, so the window never chooses a path.
//! What is read is not trusted: the size is capped, the file must be this format, and every song is checked and
//! cleaned like anything else that comes from outside (only plain `YouTube` songs are taken: no file on a computer,
//! no address, no picture). Nothing is sent anywhere.

// Tauri hands a command its arguments by value (`AppHandle`): the signature is not ours to change.
#![allow(clippy::needless_pass_by_value)]

use crate::dto::TrackDto;
use crate::state::{AppState, now_ms, with_library};
use pyxee_sources::key::{self, TrackKey};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use ts_rs::TS;

const FORMAT: &str = "pyxee-playlist";
const VERSION: u32 = 1;
/// The most a playlist file may weigh (5 000 songs are about 1 MB).
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_SONGS: usize = 5_000;
const MAX_TEXT_CHARS: usize = 300;
const MAX_NAME_CHARS: usize = pyxee_library::MAX_NAME_LEN;
/// No song is longer than a day.
const MAX_DURATION_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaylistFile {
    format: String,
    version: u32,
    #[serde(default)]
    name: String,
    tracks: Vec<FileSong>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileSong {
    key: String,
    title: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    duration_ms: Option<u64>,
}

/// What an import made, for the note that tells the listener.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportedPlaylistDto {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    /// Songs in the new playlist.
    pub songs: u32,
    /// Entries of the file that were left out (not a plain `YouTube` song, no title, or the same song again).
    pub skipped: u32,
}

struct Imported {
    name: String,
    songs: Vec<TrackDto>,
    skipped: usize,
}

fn is_youtube(key_text: &str) -> bool {
    matches!(key::parse(key_text), Some(TrackKey::Youtube(_)))
}

/// Text without control characters, trimmed and at most `max` characters.
fn plain(text: &str, max: usize) -> String {
    let cleaned: String = text.chars().filter(|c| !c.is_control()).collect();
    cleaned.trim().chars().take(max).collect()
}

/// The file's text for these songs, and how many songs were left out: the songs on this computer, because a path
/// means nothing on another one and is not something to hand out.
fn export_text(name: &str, songs: &[TrackDto]) -> Result<(String, usize), String> {
    let tracks: Vec<FileSong> = songs
        .iter()
        .filter(|t| is_youtube(&t.key))
        .map(|t| FileSong {
            key: t.key.clone(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            duration_ms: t.duration_ms,
        })
        .collect();
    let skipped = songs.len() - tracks.len();
    let file = PlaylistFile {
        format: FORMAT.into(),
        version: VERSION,
        name: name.to_string(),
        tracks,
    };
    let text = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    Ok((text, skipped))
}

/// Reads the text of a playlist file, keeping only what can be trusted.
fn import_text(text: &str) -> Result<Imported, String> {
    let not_ours = || "That file is not a Pyxee playlist.".to_string();
    let file: PlaylistFile = serde_json::from_str(text).map_err(|_| not_ours())?;
    if file.format != FORMAT {
        return Err(not_ours());
    }
    if file.version != VERSION {
        return Err("That playlist is in a format this Pyxee cannot read.".into());
    }
    if file.tracks.len() > MAX_SONGS {
        return Err(format!("That playlist has more than {MAX_SONGS} songs."));
    }
    let total = file.tracks.len();
    let mut seen = HashSet::new();
    let mut songs = Vec::new();
    for s in file.tracks {
        let title = plain(&s.title, MAX_TEXT_CHARS);
        if title.is_empty() || !is_youtube(&s.key) || !seen.insert(s.key.clone()) {
            continue;
        }
        let artist = plain(&s.artist, MAX_TEXT_CHARS);
        songs.push(TrackDto {
            key: s.key,
            title,
            artist: if artist.is_empty() {
                pyxee_core::UNKNOWN_ARTIST.to_string()
            } else {
                artist
            },
            duration_ms: s.duration_ms.filter(|ms| *ms <= MAX_DURATION_MS),
            // A picture address from a file is not followed: the cover is filled in when the song plays.
            thumbnail: None,
        });
    }
    if songs.is_empty() {
        return Err("That file has no songs Pyxee can play.".into());
    }
    let name = plain(&file.name, MAX_NAME_CHARS);
    Ok(Imported {
        name: if name.is_empty() {
            "Imported playlist".to_string()
        } else {
            name
        },
        skipped: total - songs.len(),
        songs,
    })
}

/// The name a saved file is offered under: letters, digits, spaces, `-` and `_` only.
fn file_stem(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem: String = stem.trim().chars().take(60).collect();
    if stem.is_empty() {
        "playlist".to_string()
    } else {
        stem
    }
}

/// A place on this computer: a network path would be contacted with the person's credentials just to look for it.
fn is_local(path: &Path) -> bool {
    key::parse(&key::file(path)).is_some()
}

/// The text of a file chosen by the listener, refusing anything that is not a plain, small text file.
fn read_capped(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Could not open the file: {e}"))?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("That is not a file.".into());
    }
    let too_big = || "That file is too big to be a playlist.".to_string();
    if meta.len() > MAX_FILE_BYTES {
        return Err(too_big());
    }
    let mut text = String::new();
    // One byte more than the cap is read, so a file that grew meanwhile is noticed.
    file.take(MAX_FILE_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|_| "That file is not a text file.".to_string())?;
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err(too_big());
    }
    Ok(text)
}

/// Playlist > Export: writes the playlist to a `.json` file chosen in the system's save dialog. Resolves to what to
/// tell the listener, or `None` when the dialog was closed without a choice.
#[tauri::command]
pub async fn playlist_export(app: AppHandle, id: i64) -> Result<Option<String>, String> {
    let playlist = {
        let state = app.state::<AppState>();
        let lib = state
            .library
            .lock()
            .map_err(|_| "The library is unavailable.".to_string())?;
        lib.playlist(id)?
    };
    let songs: Vec<TrackDto> = playlist.entries.iter().map(|e| e.track.clone()).collect();
    let (text, skipped) = export_text(&playlist.name, &songs)?;
    if skipped == songs.len() {
        return Err(
            "No song of this playlist can be exported: songs on this PC are left out.".into(),
        );
    }
    let file_name = format!("{}.json", file_stem(&playlist.name));
    let asker = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        asker
            .dialog()
            .file()
            .set_title("Save the playlist")
            .set_file_name(file_name)
            .add_filter("Pyxee playlist", &["json"])
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    if !is_local(&path) {
        return Err("Choose a place on this PC.".into());
    }
    let shown = path.file_name().map_or_else(
        || "the file".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    tauri::async_runtime::spawn_blocking(move || std::fs::write(&path, text))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Could not save the file: {e}"))?;
    let kept = songs.len() - skipped;
    let noun = if kept == 1 { "song" } else { "songs" };
    let left_out = if skipped > 0 {
        format!(" {skipped} on this PC left out.")
    } else {
        String::new()
    };
    Ok(Some(format!("Saved {kept} {noun} to {shown}.{left_out}")))
}

/// Library > Import playlist: asks for a playlist file in the system's dialog and makes a new playlist of it.
/// `None` when the dialog was closed without a choice.
#[tauri::command]
pub async fn playlist_import(app: AppHandle) -> Result<Option<ImportedPlaylistDto>, String> {
    let asker = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        asker
            .dialog()
            .file()
            .set_title("Choose a playlist file")
            .add_filter("Pyxee playlist", &["json"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    if !is_local(&path) {
        return Err("Choose a file on this PC.".into());
    }
    let text = tauri::async_runtime::spawn_blocking(move || read_capped(&path))
        .await
        .map_err(|e| e.to_string())??;
    let imported = import_text(&text)?;
    let id = with_library(&app, |l| {
        l.create_playlist(&imported.name, &imported.songs, now_ms())
    })?;
    Ok(Some(ImportedPlaylistDto {
        id,
        name: imported.name,
        songs: u32::try_from(imported.songs.len()).unwrap_or(u32::MAX),
        skipped: u32::try_from(imported.skipped).unwrap_or(u32::MAX),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(key: &str, title: &str) -> TrackDto {
        TrackDto {
            key: key.into(),
            title: title.into(),
            artist: "Artist".into(),
            duration_ms: Some(1000),
            thumbnail: Some("https://i.ytimg.com/vi/x/mq.jpg".into()),
        }
    }

    #[test]
    fn songs_on_this_pc_stay_out_of_a_file_and_the_rest_comes_back() {
        let songs = [
            song("yt:abc", "One"),
            song("file:C:\\Music\\two.mp3", "Two"),
        ];
        let (text, skipped) = export_text("Road trip", &songs).unwrap();
        assert_eq!(skipped, 1);
        assert!(!text.contains("Music"));
        assert!(!text.contains("ytimg"));
        let back = import_text(&text).unwrap();
        assert_eq!(back.name, "Road trip");
        assert_eq!(back.songs.len(), 1);
        assert_eq!(back.songs[0].key, "yt:abc");
        assert_eq!(back.songs[0].thumbnail, None);
        assert_eq!(back.skipped, 0);
    }

    #[test]
    fn what_a_file_says_is_checked_and_cleaned() {
        let text = r#"{"format":"pyxee-playlist","version":1,"name":"  Mix\u0007  ","tracks":[
            {"key":"yt:good_ID-1","title":"Fine\u0000 song","artist":"","durationMs":999999999999},
            {"key":"yt:good_ID-1","title":"Again","artist":"x"},
            {"key":"yt:../x","title":"Path","artist":"x"},
            {"key":"file:/etc/passwd","title":"File","artist":"x"},
            {"key":"https://evil.example/a","title":"Address","artist":"x"},
            {"key":"yt:ok2","title":"   ","artist":"x"}]}"#;
        let got = import_text(text).unwrap();
        assert_eq!(got.name, "Mix");
        assert_eq!(got.songs.len(), 1);
        assert_eq!(got.songs[0].title, "Fine song");
        assert_eq!(got.songs[0].artist, pyxee_core::UNKNOWN_ARTIST);
        assert_eq!(got.songs[0].duration_ms, None);
        assert_eq!(got.skipped, 5);
    }

    #[test]
    fn a_file_of_another_kind_is_refused() {
        assert!(import_text("not json").is_err());
        assert!(import_text(r#"{"format":"other","version":1,"tracks":[]}"#).is_err());
        assert!(import_text(r#"{"format":"pyxee-playlist","version":2,"tracks":[]}"#).is_err());
        assert!(import_text(r#"{"format":"pyxee-playlist","version":1,"tracks":[]}"#).is_err());
    }

    #[test]
    fn a_saved_file_name_has_nothing_that_could_move_it() {
        assert_eq!(file_stem("../../Mix: 1"), "______Mix_ 1");
        assert_eq!(file_stem("   "), "playlist");
    }
}
