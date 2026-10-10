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

impl Track {
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
