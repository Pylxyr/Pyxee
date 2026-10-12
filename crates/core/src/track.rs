use serde::{Deserialize, Serialize};

/// A playable item, independent of where it comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    /// Opaque key a `MusicSource` understands, e.g. `yt:<video id>` or `file:<path>`.
    pub key: String,
    pub title: String,
    pub artist: String,
    pub duration_ms: Option<u64>,
    pub thumbnail: Option<String>,
}

/// What search shows when it does not know the artist.
pub const UNKNOWN_ARTIST: &str = "Unknown artist";

/// Facts learned about a song after it was first listed, e.g. when its stream is resolved.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrackDetails {
    pub artist: Option<String>,
    pub duration_ms: Option<u64>,
    pub thumbnail: Option<String>,
}

impl Track {
    pub fn artist_known(&self) -> bool {
        let a = self.artist.trim();
        !a.is_empty() && !a.eq_ignore_ascii_case(UNKNOWN_ARTIST)
    }

    /// Fills in what is missing and returns whether anything changed. A known artist and a known
    /// duration are never overwritten (search results may be better curated than the stream's
    /// metadata); the thumbnail is replaced by the resolver's, which is the largest one on offer.
    pub fn fill(&mut self, d: &TrackDetails) -> bool {
        let before = self.clone();
        if let Some(a) = d.artist.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
            if !self.artist_known() {
                self.artist = a.to_string();
            }
        }
        if let Some(ms) = d.duration_ms.filter(|ms| *ms > 0) {
            if self.duration_ms.is_none_or(|cur| cur == 0) {
                self.duration_ms = Some(ms);
            }
        }
        if let Some(t) = d.thumbnail.as_ref().filter(|t| !t.is_empty()) {
            self.thumbnail = Some(t.clone());
        }
        *self != before
    }

    pub fn new(key: &str, title: &str, artist: &str) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            artist: artist.into(),
            duration_ms: None,
            thumbnail: None,
        }
    }
}
