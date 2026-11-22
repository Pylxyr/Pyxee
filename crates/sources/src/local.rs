//! Local files: `file:<path>` keys. This resolves a file for playback, and finds the songs of the folders the listener
//! chose (`scan`) with the title, artist, album and length their tags give.

use crate::{
    MusicSource, SearchFilter, SearchItem, SourceError, StreamInfo, TrackInfo, UNKNOWN_ARTIST,
};
use lofty::config::ParseOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// The file extensions the built-in decoder plays (see `scripts/build-ffmpeg.sh`: its demuxers and decoders).
pub const EXTENSIONS: [&str; 12] = [
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "m4b", "aac", "wav", "aif", "aiff", "mka",
];

/// Folders are searched this deep; a deeper tree is more likely a loop of junctions than a music collection.
const MAX_DEPTH: usize = 24;

/// A song of the on-disk library: what its tags said when the file was last read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalTrack {
    /// The absolute path of the file.
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: Option<u64>,
    /// The file's modified time (seconds since 1970) and size when it was read: it is read again only when one changed.
    pub modified: u64,
    pub size: u64,
}

impl LocalTrack {
    #[must_use]
    pub fn key(&self) -> String {
        key_for(Path::new(&self.path))
    }

    #[must_use]
    pub fn to_info(&self) -> TrackInfo {
        TrackInfo {
            key: self.key(),
            title: self.title.clone(),
            artist: self.artist.clone(),
            duration_ms: self.duration_ms,
            thumbnail: Some(cover_url(Path::new(&self.path), self.modified)),
        }
    }

    /// Whether the song's title, artist or album contains `needle`, which the caller has lower-cased.
    #[must_use]
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || [&self.title, &self.artist, &self.album]
                .iter()
                .any(|field| field.to_lowercase().contains(needle))
    }
}

/// Artist, then album, then title, ignoring case: how a music collection reads.
pub fn sort_tracks(tracks: &mut [LocalTrack]) {
    tracks.sort_by_cached_key(|t| {
        (
            t.artist.to_lowercase(),
            t.album.to_lowercase(),
            t.title.to_lowercase(),
            t.path.to_lowercase(),
        )
    });
}

/// Whether the path names a file the built-in decoder can play, going by its extension.
#[must_use]
pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// The name of the URL scheme the window loads a local song's cover from (the app answers it, see `read_cover`).
pub const COVER_SCHEME: &str = "pyxeecover";

/// The most a cover picture may weigh to be shown (a few MB is a large cover; more is not a cover).
const COVER_MAX_BYTES: u64 = 16 * 1024 * 1024;

/// A picture file in a song's folder that counts as its cover, best name first. Compared in lower case.
const FOLDER_COVERS: [&str; 12] = [
    "cover.jpg",
    "cover.jpeg",
    "cover.png",
    "cover.webp",
    "folder.jpg",
    "folder.jpeg",
    "folder.png",
    "front.jpg",
    "front.jpeg",
    "front.png",
    "albumart.jpg",
    "albumart.png",
];

/// A cover picture: its bytes and its media type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

fn hex_encode(text: &str) -> String {
    use std::fmt::Write;
    text.bytes().fold(String::with_capacity(text.len() * 2), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

fn hex_decode(hex: &str) -> Option<String> {
    if hex.is_empty() || hex.len() % 2 == 1 || !hex.is_ascii() {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}

/// The address the window loads the cover of the song at `path` from. It names the file in hex (nothing in it needs
/// escaping) and carries `version` (the file's modified time), so a song whose picture was changed is asked for again.
/// Windows' webview serves a custom scheme from `http://<scheme>.localhost`, the others from `<scheme>://localhost`.
#[must_use]
pub fn cover_url(path: &Path, version: u64) -> String {
    let name = hex_encode(&path.display().to_string());
    if cfg!(windows) {
        format!("http://{COVER_SCHEME}.localhost/{name}?v={version}")
    } else {
        format!("{COVER_SCHEME}://localhost/{name}?v={version}")
    }
}

/// Whether `url` is the address of a local cover (see `cover_url`), whichever form the webview uses.
#[must_use]
pub fn is_cover_url(url: &str) -> bool {
    cover_url_name(url).is_some()
}

fn cover_url_name(url: &str) -> Option<&str> {
    let rest = [
        format!("http://{COVER_SCHEME}.localhost/"),
        format!("https://{COVER_SCHEME}.localhost/"),
        format!("{COVER_SCHEME}://localhost/"),
    ]
    .iter()
    .find_map(|prefix| url.strip_prefix(prefix.as_str()))?;
    rest.split(['?', '#']).next()
}

/// The song a cover address names, and only a song: the path must be a local (not network) absolute path to a file
/// the player plays by its extension. The webview can ask for any address, so nothing else is ever opened for it.
#[must_use]
pub fn cover_url_path(url: &str) -> Option<PathBuf> {
    let text = hex_decode(cover_url_name(url)?)?;
    crate::key::parse(&format!("file:{text}"))?;
    let path = PathBuf::from(text);
    (path.is_absolute() && is_audio_file(&path)).then_some(path)
}

/// The media type of a picture by its first bytes, for the formats a webview shows. `None` for anything else, so a
/// file that merely has a picture's name is never handed over.
fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// The picture inside the file's tags: the front cover when it has one, else the first picture.
fn embedded_cover(path: &Path) -> Option<Cover> {
    let tagged = Probe::open(path)
        .ok()?
        .options(ParseOptions::new().read_cover_art(true))
        .guess_file_type()
        .ok()?
        .read()
        .ok()?;
    let pictures: Vec<&lofty::picture::Picture> = tagged
        .tags()
        .iter()
        .flat_map(|tag| tag.pictures())
        .filter(|p| p.data().len() as u64 <= COVER_MAX_BYTES)
        .collect();
    let picked = pictures
        .iter()
        .find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront)
        .or_else(|| pictures.first())?;
    let mime = sniff_image(picked.data())?;
    Some(Cover {
        mime,
        bytes: picked.data().to_vec(),
    })
}

/// A picture beside the song that is named like a cover (`cover.jpg`, `folder.png`, ...), by the best name first.
fn folder_cover(path: &Path) -> Option<Cover> {
    let names: Vec<(String, PathBuf)> = std::fs::read_dir(path.parent()?)
        .ok()?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| Some((e.file_name().to_str()?.to_lowercase(), e.path())))
        .collect();
    for wanted in FOLDER_COVERS {
        let Some((_, file)) = names.iter().find(|(name, _)| name == wanted) else {
            continue;
        };
        let Ok(meta) = std::fs::metadata(file) else {
            continue;
        };
        if meta.len() > COVER_MAX_BYTES {
            continue;
        }
        let Ok(bytes) = std::fs::read(file) else {
            continue;
        };
        if let Some(mime) = sniff_image(&bytes) {
            return Some(Cover { mime, bytes });
        }
    }
    None
}

/// The cover of the song at `path`: the picture in its tags, else a cover picture in its folder. `None` when there is
/// neither, or when `path` is not a song the player plays. Reads the file, so call it off the main thread.
#[must_use]
pub fn read_cover(path: &Path) -> Option<Cover> {
    if !path.is_absolute() || !is_audio_file(path) || !path.is_file() {
        return None;
    }
    // A broken file must not take the app down: the parser's panic ends up as a song without a cover.
    std::panic::catch_unwind(|| embedded_cover(path))
        .ok()
        .flatten()
        .or_else(|| folder_cover(path))
}

struct Tags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<u64>,
}

fn clean(text: &str) -> Option<String> {
    let t = text.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// What the file's tags and properties say. `None` when it cannot be read as audio at all.
fn read_tags(path: &Path) -> Option<Tags> {
    // The pictures are skipped: they are most of a file's tag and nothing here shows them.
    let tagged = Probe::open(path)
        .ok()?
        .options(ParseOptions::new().read_cover_art(false))
        .guess_file_type()
        .ok()?
        .read()
        .ok()?;
    let duration = tagged.properties().duration().as_millis();
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    Some(Tags {
        title: tag.and_then(|t| t.title().and_then(|s| clean(&s))),
        artist: tag.and_then(|t| {
            t.artist()
                .and_then(|s| clean(&s))
                .or_else(|| t.get_string(ItemKey::AlbumArtist).and_then(clean))
        }),
        album: tag.and_then(|t| t.album().and_then(|s| clean(&s))),
        duration_ms: u64::try_from(duration).ok().filter(|ms| *ms > 0),
    })
}

/// A song from the file at `path`, whose modified time and size are `modified` and `size`. A file whose tags cannot be
/// read is still a song, titled from its name.
#[must_use]
pub fn read_track(path: &Path, modified: u64, size: u64) -> LocalTrack {
    // A broken file must not take the scan down with it: the parser's panic ends up as a song without tags.
    let tags = std::panic::catch_unwind(|| read_tags(path)).ok().flatten();
    let stem = path.file_stem().map_or_else(
        || "Unknown".to_string(),
        |s| s.to_string_lossy().into_owned(),
    );
    let (title, artist, album, duration_ms) = match tags {
        Some(t) => (t.title, t.artist, t.album, t.duration_ms),
        None => (None, None, None, None),
    };
    LocalTrack {
        path: path.display().to_string(),
        title: title.unwrap_or(stem),
        artist: artist.unwrap_or_else(|| UNKNOWN_ARTIST.to_string()),
        album: album.unwrap_or_default(),
        duration_ms,
        modified,
        size,
    }
}

fn stamp(meta: &std::fs::Metadata) -> (u64, u64) {
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    (modified, meta.len())
}

/// Folders that are never music and often huge or locked (Windows keeps one on every drive).
fn is_skipped_folder(name: &str) -> bool {
    name.starts_with('.')
        || name.eq_ignore_ascii_case("$RECYCLE.BIN")
        || name.eq_ignore_ascii_case("System Volume Information")
}

/// What a scan found, and what it could not look at.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Scan {
    pub tracks: Vec<LocalTrack>,
    /// Chosen folders that could not be listed: a drive that is not connected, a network location that is down, a
    /// folder that was renamed or deleted. The songs `previous` held under them are kept in `tracks`, so a drive that
    /// is only unplugged does not empty the library.
    pub unreachable: Vec<PathBuf>,
    /// How many songs of `previous` are no longer there, in the folders that could be listed.
    pub removed: usize,
}

/// The songs in `folders` and everything below them, read from their tags. A file that has not changed since
/// `previous` was made (same size and modified time) is not read again, so a rescan of a large collection takes a
/// moment. Links are not followed (a folder that links back to its parent would never end). A folder that cannot be
/// listed keeps the songs it held in `previous` (see `Scan::unreachable`) instead of losing them. `stop` is asked
/// between files; when it says so the scan ends and returns `None`.
pub fn scan_folders(
    folders: &[PathBuf],
    previous: &[LocalTrack],
    stop: &dyn Fn() -> bool,
) -> Option<Scan> {
    let known: HashMap<&str, &LocalTrack> = previous.iter().map(|t| (t.path.as_str(), t)).collect();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut found: Vec<LocalTrack> = Vec::new();
    let mut unreachable: Vec<PathBuf> = Vec::new();
    // Folders that could not be listed: what `previous` held under them stays.
    let mut unlisted: Vec<PathBuf> = Vec::new();
    for root in folders {
        if !root.is_absolute() {
            continue;
        }
        let mut pending = vec![(root.clone(), 0usize)];
        while let Some((dir, depth)) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                if depth == 0 {
                    unreachable.push(dir.clone());
                }
                unlisted.push(dir);
                continue;
            };
            for entry in entries.flatten() {
                if stop() {
                    return None;
                }
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                let path = entry.path();
                if kind.is_dir() {
                    let skipped = entry.file_name().to_str().is_some_and(is_skipped_folder);
                    if !skipped && depth < MAX_DEPTH {
                        pending.push((path, depth + 1));
                    }
                } else if kind.is_file() && is_audio_file(&path) && seen.insert(path.clone()) {
                    if crate::key::parse(&key_for(&path)).is_none() {
                        continue; // a network or device path: never touched
                    }
                    let Ok(meta) = entry.metadata() else {
                        continue;
                    };
                    let (modified, size) = stamp(&meta);
                    let text = path.display().to_string();
                    match known.get(text.as_str()) {
                        Some(old) if old.modified == modified && old.size == size => {
                            found.push((*old).clone());
                        }
                        _ => found.push(read_track(&path, modified, size)),
                    }
                }
            }
        }
    }
    if !unlisted.is_empty() {
        let have: HashSet<&str> = found.iter().map(|t| t.path.as_str()).collect();
        let kept: Vec<LocalTrack> = previous
            .iter()
            .filter(|t| !have.contains(t.path.as_str()))
            .filter(|t| unlisted.iter().any(|d| Path::new(&t.path).starts_with(d)))
            .cloned()
            .collect();
        found.extend(kept);
    }
    let now: HashSet<&str> = found.iter().map(|t| t.path.as_str()).collect();
    let removed = previous
        .iter()
        .filter(|t| !now.contains(t.path.as_str()))
        .count();
    sort_tracks(&mut found);
    Some(Scan {
        tracks: found,
        unreachable,
        removed,
    })
}

/// `scan_folders` without the report: just the songs.
pub fn scan(
    folders: &[PathBuf],
    previous: &[LocalTrack],
    stop: &dyn Fn() -> bool,
) -> Option<Vec<LocalTrack>> {
    scan_folders(folders, previous, stop).map(|s| s.tracks)
}

/// Why the file at `path` cannot be opened because it is gone, in words for the listener (it follows "so it was
/// skipped:"). `None` when the file is there, or cannot be told to be gone (no permission, say).
#[must_use]
pub fn missing_reason(path: &Path) -> Option<String> {
    match std::fs::metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        _ => return None,
    }
    let folder = path.parent()?;
    Some(if folder.is_dir() {
        format!(
            "its file is no longer in {} (moved, renamed or deleted)",
            folder.display()
        )
    } else {
        format!(
            "its folder {} can't be found (is the drive or network location connected?)",
            folder.display()
        )
    })
}

#[derive(Default)]
pub struct LocalSource;

#[must_use]
pub fn key_for(path: &Path) -> String {
    crate::key::file(path)
}

/// A track entry for a local file, titled from its file name until tag reading arrives.
#[must_use]
pub fn track_for(path: &Path) -> TrackInfo {
    TrackInfo {
        key: key_for(path),
        title: path
            .file_stem()
            .map_or_else(|| "Unknown".into(), |s| s.to_string_lossy().into_owned()),
        artist: "Local file".into(),
        duration_ms: None,
        thumbnail: Some(cover_url(path, 0)),
    }
}

impl MusicSource for LocalSource {
    fn search(&self, _q: &str, _f: SearchFilter) -> Result<Vec<SearchItem>, SourceError> {
        Ok(Vec::new())
    }

    fn resolve_stream(&self, key: &str) -> Result<StreamInfo, SourceError> {
        let Some(crate::key::TrackKey::File(path)) = crate::key::parse(key) else {
            return Err(SourceError::Failed(format!("not a local key: {key}")));
        };
        if !Path::new(path).is_absolute() {
            return Err(SourceError::Failed("not a local file path".into()));
        }
        if !Path::new(path).is_file() {
            let why = missing_reason(Path::new(path))
                .unwrap_or_else(|| "its file can't be opened".to_string());
            return Err(SourceError::Unavailable(format!(
                "the file {path} is missing: {why}"
            )));
        }
        let ext = Path::new(path)
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        Ok(StreamInfo {
            url: path.to_string(),
            ext,
            ..Default::default()
        })
    }

    fn related(&self, _key: &str, _count: usize) -> Result<Vec<TrackInfo>, SourceError> {
        Ok(Vec::new())
    }
    fn suggest(&self, _prefix: &str) -> Result<Vec<String>, SourceError> {
        Ok(Vec::new())
    }
    fn supported_filters(&self) -> Vec<SearchFilter> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_an_existing_file_and_reports_a_missing_one_clearly() {
        let dir = std::env::temp_dir().join(format!("pyxee-local-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("Song One.FLAC");
        std::fs::write(&f, b"x").unwrap();
        let s = LocalSource.resolve_stream(&key_for(&f)).unwrap();
        assert_eq!(s.ext, "flac");
        assert_eq!(s.url, f.display().to_string());
        let err = LocalSource
            .resolve_stream(&key_for(&dir.join("gone.mp3")))
            .unwrap_err();
        assert!(matches!(err, SourceError::Unavailable(m) if m.contains("missing")));
        assert_eq!(track_for(&f).title, "Song One");
    }

    fn wav(seconds: u32) -> Vec<u8> {
        let data_len = 8_000 * 2 * seconds;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data_len).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&8_000u32.to_le_bytes());
        b.extend_from_slice(&16_000u32.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&data_len.to_le_bytes());
        b.resize(b.len() + data_len as usize, 0);
        b
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pyxee-scan-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn never() -> bool {
        false
    }

    #[test]
    fn a_cover_address_names_the_song_in_hex_and_only_a_song_can_be_asked_for() {
        let song = Path::new(if cfg!(windows) {
            "C:\\Music\\Ünï cöde.flac"
        } else {
            "/music/Ünï cöde.flac"
        });
        let url = cover_url(song, 7);
        assert!(is_cover_url(&url), "{url}");
        assert!(url.ends_with("?v=7"), "{url}");
        assert_eq!(cover_url_path(&url).as_deref(), Some(song));
        // Not a song, not absolute, a network path, no name at all, not hex, or not ours: nothing is opened.
        for name in [
            hex_encode("/etc/passwd"),
            hex_encode("relative/song.mp3"),
            hex_encode("\\\\host\\share\\song.mp3"),
            hex_encode("//host/share/song.mp3"),
            String::new(),
            "zz".into(),
            "abc".into(),
        ] {
            let url = format!("{COVER_SCHEME}://localhost/{name}");
            assert_eq!(cover_url_path(&url), None, "{url}");
        }
        assert_eq!(cover_url_path("https://i.ytimg.com/vi/x/hq.jpg"), None);
        assert!(!is_cover_url("https://i.ytimg.com/vi/x/hq.jpg"));
    }

    #[test]
    fn a_cover_is_the_picture_in_the_tags_else_one_named_like_a_cover_in_the_folder() {
        let dir = scratch("cover");
        let song = dir.join("song.wav");
        std::fs::write(&song, wav(1)).unwrap();
        assert_eq!(read_cover(&song), None, "no picture anywhere");

        // A PNG header is enough for the picture to be recognised; a text file named cover.jpg is not a cover.
        std::fs::write(dir.join("cover.jpg"), b"not a picture").unwrap();
        assert_eq!(read_cover(&song), None);
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
        std::fs::write(dir.join("Folder.PNG"), png).unwrap();
        let found = read_cover(&song).unwrap();
        assert_eq!(found.mime, "image/png");
        assert_eq!(found.bytes, png);

        // The better name wins.
        let jpg = [0xFF, 0xD8, 0xFF, 0xE0, 9];
        std::fs::write(dir.join("cover.jpg"), jpg).unwrap();
        assert_eq!(read_cover(&song).unwrap().mime, "image/jpeg");

        // Only songs have covers.
        assert_eq!(read_cover(&dir.join("notes.txt")), None);
        assert_eq!(read_cover(Path::new("relative.wav")), None);
    }

    #[test]
    fn pictures_are_told_by_their_first_bytes() {
        assert_eq!(sniff_image(&[0xFF, 0xD8, 0xFF, 0]), Some("image/jpeg"));
        assert_eq!(sniff_image(b"GIF89a"), Some("image/gif"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WAVEfmt "), None);
        assert_eq!(sniff_image(b"<svg>"), None);
        assert_eq!(sniff_image(b""), None);
    }

    #[test]
    fn a_scan_finds_the_audio_files_below_the_folders_and_nothing_else() {
        let dir = scratch("finds");
        std::fs::create_dir_all(dir.join("Artist/Album")).unwrap();
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();
        std::fs::write(dir.join("Artist/Album/02 Second.WAV"), wav(1)).unwrap();
        std::fs::write(dir.join("Artist/01 First.wav"), wav(2)).unwrap();
        std::fs::write(dir.join("cover.jpg"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        std::fs::write(dir.join(".hidden/secret.wav"), wav(1)).unwrap();
        // Not audio at all, but named like it: still listed, titled from its name.
        std::fs::write(dir.join("broken.mp3"), b"not audio").unwrap();

        let found = scan(std::slice::from_ref(&dir), &[], &never).unwrap();
        let titles: Vec<&str> = found.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(
            titles,
            ["01 First", "02 Second", "broken"],
            "sorted, no .jpg, .txt or hidden folder"
        );
        let first = found.iter().find(|t| t.title == "01 First").unwrap();
        assert_eq!(first.artist, UNKNOWN_ARTIST);
        assert!(
            matches!(first.duration_ms, Some(1_990..=2_010)),
            "{:?}",
            first.duration_ms
        );
        assert!(found
            .iter()
            .find(|t| t.title == "broken")
            .unwrap()
            .duration_ms
            .is_none());
        assert_eq!(first.key(), key_for(&dir.join("Artist").join("01 First.wav")));
        assert_eq!(first.to_info().key, first.key());
    }

    #[test]
    fn a_rescan_reads_again_only_what_changed_and_a_file_in_two_folders_is_listed_once() {
        let dir = scratch("rescan");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let a = dir.join("a.wav");
        std::fs::write(&a, wav(1)).unwrap();
        let first = scan(&[dir.clone(), dir.join("sub"), dir.clone()], &[], &never).unwrap();
        assert_eq!(first.len(), 1);

        // Pretend what the scan remembered: a title that no tag gives. An unchanged file keeps it.
        let mut remembered = first.clone();
        remembered[0].title = "Remembered".into();
        let again = scan(std::slice::from_ref(&dir), &remembered, &never).unwrap();
        assert_eq!(
            again[0].title, "Remembered",
            "an unchanged file is not read again"
        );

        // A file of another size is read again.
        std::fs::write(&a, wav(2)).unwrap();
        let changed = scan(std::slice::from_ref(&dir), &remembered, &never).unwrap();
        assert_eq!(changed[0].title, "a");
        // A file that is gone is gone from the result.
        std::fs::remove_file(&a).unwrap();
        assert!(scan(&[dir], &changed, &never).unwrap().is_empty());
    }

    #[test]
    fn a_folder_that_cannot_be_listed_keeps_its_songs_and_deleted_files_are_counted() {
        let dir = scratch("unreach");
        let there = dir.join("there");
        std::fs::create_dir_all(&there).unwrap();
        std::fs::write(there.join("a.wav"), wav(1)).unwrap();
        std::fs::write(there.join("b.wav"), wav(1)).unwrap();
        let gone_root = dir.join("unplugged");
        let before = LocalTrack {
            path: gone_root.join("old.mp3").display().to_string(),
            title: "Old".into(),
            artist: "Band".into(),
            album: String::new(),
            duration_ms: None,
            modified: 1,
            size: 1,
        };
        let first = scan_folders(
            &[there.clone(), gone_root.clone()],
            std::slice::from_ref(&before),
            &never,
        )
        .unwrap();
        assert_eq!(first.unreachable, vec![gone_root.clone()]);
        assert_eq!(first.removed, 0);
        assert!(first.tracks.iter().any(|t| t.title == "Old"), "kept");
        assert_eq!(first.tracks.len(), 3);

        // A file deleted from a folder that can be listed is removed, and counted.
        std::fs::remove_file(there.join("b.wav")).unwrap();
        let again = scan_folders(&[there, gone_root], &first.tracks, &never).unwrap();
        assert_eq!(again.removed, 1);
        assert_eq!(again.tracks.len(), 2);
        assert_eq!(again.unreachable.len(), 1);
    }

    #[test]
    fn a_missing_file_is_explained_by_what_is_missing() {
        let dir = scratch("reason");
        let song = dir.join("a.wav");
        assert!(missing_reason(&song).unwrap().contains("no longer in"));
        std::fs::write(&song, wav(1)).unwrap();
        assert_eq!(missing_reason(&song), None);
        let lost = dir.join("unplugged").join("b.wav");
        assert!(missing_reason(&lost).unwrap().contains("can't be found"));
    }

    #[test]
    fn a_scan_that_is_stopped_returns_nothing_and_unusable_folders_are_skipped() {
        let dir = scratch("stop");
        std::fs::write(dir.join("a.wav"), wav(1)).unwrap();
        assert!(scan(std::slice::from_ref(&dir), &[], &|| true).is_none());
        let missing = dir.join("nowhere");
        assert_eq!(
            scan(&[PathBuf::from("relative/folder"), missing], &[], &never).unwrap(),
            Vec::new()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_back_to_a_parent_does_not_make_the_scan_loop() {
        let dir = scratch("loop");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/a.wav"), wav(1)).unwrap();
        std::os::unix::fs::symlink(&dir, dir.join("sub/back")).unwrap();
        let found = scan(&[dir], &[], &never).unwrap();
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn songs_are_found_by_title_artist_or_album_and_sorted_like_a_collection() {
        let song = |artist: &str, album: &str, title: &str| LocalTrack {
            path: format!("/music/{artist}/{title}.mp3"),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            duration_ms: None,
            modified: 0,
            size: 0,
        };
        let mut all = vec![
            song("b band", "Second", "Zed"),
            song("A Band", "Beta", "Mid"),
            song("A Band", "alpha", "Top"),
        ];
        sort_tracks(&mut all);
        let order: Vec<&str> = all.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(order, ["Top", "Mid", "Zed"]);
        assert!(all[0].matches(""));
        assert!(all[0].matches("band"));
        assert!(all[1].matches("beta"));
        assert!(all[2].matches("zed") && !all[2].matches("alpha"));
        assert!(
            is_audio_file(Path::new("/x/Song.FLAC")) && !is_audio_file(Path::new("/x/cover.jpg"))
        );
    }

    #[test]
    fn a_relative_or_network_path_is_refused_before_it_is_touched() {
        for key in [
            "file:relative/a.mp3",
            "file:a.mp3",
            r"file:\\attacker\share\a.mp3",
            "file://attacker/share/a.mp3",
            "file:",
        ] {
            assert!(
                matches!(LocalSource.resolve_stream(key), Err(SourceError::Failed(_))),
                "{key:?}"
            );
        }
    }
}
