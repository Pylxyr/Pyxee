//! Synced lyrics: LRC parsing, "which line is current" lookup, search-term cleanup and an LRCLIB client.
//! Everything except `fetch` is pure.

mod client;
mod lrc;
mod store;

pub use client::{
    Candidate, Cooldown, LRCLIB, LyricsError, Query, fetch, from_lrclib, pick_best,
    retry_after_secs,
};
pub use lrc::{clean_artist, clean_title, current_index, parse_lrc};
pub use store::{MAX_ENTRIES, Store};

use serde::{Deserialize, Serialize};

/// One line of a synced lyric: shown from `time_ms` until the next line starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub time_ms: u64,
    /// Empty for an instrumental gap; the UI draws a note.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lyrics {
    /// Time-stamped lines, sorted by time.
    Synced(Vec<Line>),
    /// Words only; shown as text without following the song.
    Plain(String),
    /// The service says there are no words.
    Instrumental,
    /// Nothing found.
    None,
}
