//! Music sources. Everything the app needs from "where music comes from" goes through
//! `MusicSource`, so yt-dlp can be swapped or supplemented without touching the player.

pub mod gate;
pub mod local;
pub mod suggest;
pub mod ytdlp;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourceError {
    #[error("You're offline, or the music service can't be reached.")]
    Offline,
    #[error("The music service is limiting requests right now. Try again in a moment.")]
    RateLimited,
    #[error("This track isn't available: {0}")]
    Unavailable(String),
    #[error("{0} was not found. Check that it is installed or set its path.")]
    ToolMissing(String),
    #[error("{0} is not supported by this source yet.")]
    Unsupported(String),
    #[error("Unexpected response from the music service: {0}")]
    Parse(String),
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
    /// Similar songs (a YouTube "mix"), used by autoplay radio. Excludes the seed itself.
    fn related(&self, key: &str, count: usize) -> Result<Vec<TrackInfo>, SourceError>;
    /// Live search suggestions for a partial query.
    fn suggest(&self, prefix: &str) -> Result<Vec<String>, SourceError>;
    /// Filters this source can really serve (the UI disables the rest).
    fn supported_filters(&self) -> Vec<SearchFilter>;
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
        if key.starts_with("file:") {
            self.local.resolve_stream(key)
        } else {
            self.remote.resolve_stream(key)
        }
    }
    fn related(&self, key: &str, count: usize) -> Result<Vec<TrackInfo>, SourceError> {
        if key.starts_with("file:") {
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
}

/// Turns yt-dlp's stderr into an error the UI can explain.
pub fn classify_stderr(tool: &str, stderr: &str) -> SourceError {
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
            "YouTube offered no audio stream this player can decode (it needs Opus in WebM or AAC-LC in M4A over HTTPS)"
                .into(),
        )
    } else if has(&["http error 403", "403: forbidden"]) {
        // After the permanent causes above: YouTube refusing this client or IP (often fixed by
        // cookies, or by waiting) is not a property of the song, so it must not skip through the queue.
        SourceError::RateLimited
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
        SourceError::Failed(format!(
            "{tool} could not read YouTube's page, so it is probably out of date: update it ({})",
            last_line(stderr)
        ))
    } else {
        SourceError::Failed(format!("{tool} failed: {}", last_line(stderr)))
    }
}

fn last_line(s: &str) -> String {
    s.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("unknown error")
        .trim()
        .to_string()
}
