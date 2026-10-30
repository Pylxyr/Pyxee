//! Music sources. Everything the app needs from "where music comes from" goes through
//! `MusicSource`, so yt-dlp can be swapped or supplemented without touching the player.

mod cover;
pub mod gate;
pub mod jsruntime;
pub mod key;
pub mod local;
mod net;
mod scrub;
pub mod suggest;
pub mod ytdlp;
pub mod ytmusic;

pub use cover::is_cover_host;
pub use scrub::scrub;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourceError {
    #[error("You're offline, or the music service can't be reached.")]
    Offline,
    #[error("The music service is limiting requests right now. Try again in a moment.")]
    RateLimited,
    /// YouTube answered HTTP 403: it refused this request (this song, this player client, or this network). Unlike a
    /// rate limit it says nothing about the requests that follow, so the gate waits for several in a row.
    #[error("YouTube refused the request (HTTP 403). Try again; if it keeps happening, update yt-dlp or sign in with cookies in Settings.")]
    Forbidden,
    #[error("This track isn't available: {0}")]
    Unavailable(String),
    #[error("{0} was not found. Check that it is installed or set its path.")]
    ToolMissing(String),
    #[error("{0} is not supported by this source yet.")]
    Unsupported(String),
    #[error("Unexpected response from the music service: {0}")]
    Parse(String),
    /// yt-dlp ran but cannot read YouTube's pages (usually: out of date). Not about one song, so the
    /// text is fixed and the detail (yt-dlp's last line) is kept only for the log.
    #[error("yt-dlp could not read YouTube's page, so it is probably out of date: update it")]
    ToolBroken(String),
    #[error("{0}")]
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchFilter {
    All,
    Songs,
    Videos,
    Albums,
    Artists,
    Playlists,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackInfo {
    /// `yt:<video id>` or `file:<path>`.
    pub key: String,
    pub title: String,
    pub artist: String,
    pub duration_ms: Option<u64>,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SearchItem {
    Track(TrackInfo),
    Playlist {
        key: String,
        title: String,
        uploader: String,
        thumbnail: Option<String>,
    },
    Artist {
        key: String,
        name: String,
        thumbnail: Option<String>,
    },
}

/// An album or single on an Explore shelf. There is no album page in Pyxee yet, so this is only what a cover tile needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlbumInfo {
    /// YouTube Music's id for the release (`MPREb_...`).
    pub id: String,
    pub title: String,
    pub artist: String,
    pub thumbnail: Option<String>,
}

/// A genre or mood of YouTube Music's "Moods & genres" page (K-Pop, Anime, Chill...). `params` is the opaque token its
/// page is asked for with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenreInfo {
    pub title: String,
    pub params: String,
}

/// A playlist as a cover tile (a genre page's playlists): what it is called, who makes it, and its cover.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistInfo {
    /// YouTube's playlist id (no `VL` prefix).
    pub id: String,
    pub title: String,
    /// Who it is by or how long it is, as the website words it; empty when it says nothing useful.
    pub subtitle: String,
    pub thumbnail: Option<String>,
}

/// One row of YouTube Music's Explore page, as a shelf of songs, albums or playlists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExploreShelf {
    Playlists {
        id: String,
        title: String,
        playlists: Vec<PlaylistInfo>,
    },
    Songs {
        id: String,
        title: String,
        tracks: Vec<TrackInfo>,
    },
    Albums {
        id: String,
        title: String,
        albums: Vec<AlbumInfo>,
    },
}

/// What search shows when it does not know the artist (same text as `pyxee_core::UNKNOWN_ARTIST`).
pub const UNKNOWN_ARTIST: &str = "Unknown artist";

/// A directly playable stream.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StreamInfo {
    /// `http(s)://...` for remote streams, a file path for local files.
    pub url: String,
    pub ext: String,
    pub acodec: Option<String>,
    pub abr_kbps: Option<f64>,
    /// HTTP headers the stream URL must be fetched with (yt-dlp's `http_headers`: User-Agent etc.).
    /// Empty for local files.
    pub headers: Vec<(String, String)>,
    /// What the resolver learned about the song, so the UI can fill in what search left out.
    pub meta: StreamMeta,
}

/// Song details reported while resolving a stream; every field may be unknown.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StreamMeta {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration_secs: Option<f64>,
    pub thumbnail: Option<String>,
}

pub trait MusicSource: Send + Sync {
    fn search(&self, query: &str, filter: SearchFilter) -> Result<Vec<SearchItem>, SourceError>;
    fn resolve_stream(&self, key: &str) -> Result<StreamInfo, SourceError>;
    /// Similar songs (a YouTube "mix"), used by autoplay radio. Excludes the seed itself. Songs only: no videos or
    /// other media, whatever the seed is (a music video seed gets songs too), unless the mix holds hardly any songs.
    fn related(&self, key: &str, count: usize) -> Result<Vec<TrackInfo>, SourceError>;
    /// Live search suggestions for a partial query.
    fn suggest(&self, prefix: &str) -> Result<Vec<String>, SourceError>;
    /// Filters this source can really serve (the UI disables the rest).
    fn supported_filters(&self) -> Vec<SearchFilter>;
    /// The shelves of YouTube Music's Explore page (trending songs, new releases...). Sources without such a page
    /// say so with `Unsupported`.
    fn explore(&self) -> Result<Vec<ExploreShelf>, SourceError> {
        Err(SourceError::Unsupported("Explore".into()))
    }
    /// The genres and moods YouTube Music lists on its "Moods & genres" page.
    fn genres(&self) -> Result<Vec<GenreInfo>, SourceError> {
        Err(SourceError::Unsupported("Genres".into()))
    }
    /// The shelves of one genre's page (`params` from `genres`): playlists and songs, no albums.
    fn genre(&self, _params: &str) -> Result<Vec<ExploreShelf>, SourceError> {
        Err(SourceError::Unsupported("Genres".into()))
    }
    /// The songs of a YouTube Music playlist (the first page of it), in order.
    fn playlist_tracks(&self, _id: &str) -> Result<Vec<TrackInfo>, SourceError> {
        Err(SourceError::Unsupported("Playlists".into()))
    }
}

/// Routes by key prefix: `yt:` to yt-dlp, `file:` to local files. Search and suggestions go to yt-dlp.
pub struct CompositeSource {
    pub remote: ytdlp::YtDlpSource,
    pub local: local::LocalSource,
}

impl MusicSource for CompositeSource {
    fn search(&self, query: &str, filter: SearchFilter) -> Result<Vec<SearchItem>, SourceError> {
        self.remote.search(query, filter)
    }
    fn resolve_stream(&self, key: &str) -> Result<StreamInfo, SourceError> {
        if key::is_file(key) {
            self.local.resolve_stream(key)
        } else {
            self.remote.resolve_stream(key)
        }
    }
    fn related(&self, key: &str, count: usize) -> Result<Vec<TrackInfo>, SourceError> {
        if key::is_file(key) {
            Ok(Vec::new())
        } else {
            self.remote.related(key, count)
        }
    }
    fn suggest(&self, prefix: &str) -> Result<Vec<String>, SourceError> {
        self.remote.suggest(prefix)
    }
    fn supported_filters(&self) -> Vec<SearchFilter> {
        self.remote.supported_filters()
    }
    fn explore(&self) -> Result<Vec<ExploreShelf>, SourceError> {
        self.remote.explore()
    }
    fn genres(&self) -> Result<Vec<GenreInfo>, SourceError> {
        self.remote.genres()
    }
    fn genre(&self, params: &str) -> Result<Vec<ExploreShelf>, SourceError> {
        self.remote.genre(params)
    }
    fn playlist_tracks(&self, id: &str) -> Result<Vec<TrackInfo>, SourceError> {
        self.remote.playlist_tracks(id)
    }
}

/// Turns yt-dlp's stderr into an error the UI can explain.
pub fn classify_stderr(tool: &str, stderr: &str) -> SourceError {
    let tool = program_name(std::path::Path::new(tool));
    let low = stderr.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| low.contains(n));
    if has(&[
        "http error 429",
        "too many requests",
        "not a bot", // "Sign in to confirm you're not a bot" (not the age gate, see below)
        "rate-limit",
        "rate limit",
    ]) {
        SourceError::RateLimited
    } else if has(&[
        "getaddrinfo",
        "temporary failure in name resolution",
        "network is unreachable",
        "timed out",
        "urlopen error",
        "connection refused",
        "connection reset",
        "no address associated",
        "name or service not known",
        "failed to resolve",
    ]) {
        SourceError::Offline
    } else if has(&[
        "video unavailable",
        "private video",
        "this video is not available",
        "has been removed",
        "not made this video available",
        "age-restricted",
        "confirm your age",
        "members-only",
        "this video is unavailable",
    ]) {
        SourceError::Unavailable(last_line(stderr))
    } else if has(&["requested format is not available"]) {
        // Our format selector only allows what the built-in decoder can play (see
        // `ytdlp::FORMAT_SELECTOR`); there is no other decoder to fall back on.
        SourceError::Failed(
            "YouTube offered no audio stream this player can decode (it needs Opus, AAC-LC, Vorbis, FLAC, MP3 or ALAC over HTTPS). An out-of-date yt-dlp is a common cause: open Settings > Updates and choose Update yt-dlp"
                .into(),
        )
    } else if has(&["http error 403", "403: forbidden"]) {
        // After the permanent causes above: YouTube refusing this client or IP (often fixed by
        // cookies, or by waiting) is not a property of the song, so it must not skip through the queue. It is not a
        // rate limit either (see `SourceError::Forbidden`).
        SourceError::Forbidden
    } else if has(&[
        "http error 500",
        "http error 502",
        "http error 503",
        "http error 504",
        "bad gateway",
        "service unavailable",
        "gateway time-out",
    ]) {
        // YouTube's own servers failing: transient, the song is fine.
        SourceError::Offline
    } else if has(&[
        "unable to extract",
        "nsig extraction failed",
        "signature extraction failed",
    ]) {
        SourceError::ToolBroken(last_line(stderr))
    } else {
        SourceError::Failed(format!("{tool} failed: {}", last_line(stderr)))
    }
}

/// The last line of yt-dlp's output, with cookie locations and profile folders replaced.
fn last_line(s: &str) -> String {
    scrub(
        s.lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("unknown error")
            .trim(),
    )
}

/// Only the file name of a program, so a full path (which holds the user name) never reaches an
/// error message or a log line.
pub(crate) fn program_name(path: &std::path::Path) -> String {
    path.file_name().map_or_else(
        || "the program".to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}
