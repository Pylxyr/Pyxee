//! Synced lyrics: LRC parsing, "which line is current" lookup, search-term cleanup and an LRCLIB client.
//! Everything except `fetch` is pure.

// Pedantic lints this project does not follow, allowed once per crate root. (A workspace-level allow loses to the
// `-D clippy::pedantic` that is passed on the command line, so it has to be an attribute.) Each is a trade-off for
// application code: `# Errors` / `# Panics` sections and `#[must_use]` on internal items, long match-heavy functions,
// flag structs, short names in numeric code, and the casts between sample counts, milliseconds and pixels (their
// ranges are bounded by what the device and the files can hold) are not worth the noise.
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::too_many_lines,
    clippy::struct_excessive_bools,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

mod client;
mod lrc;

pub use client::{
    Candidate, Cooldown, LRCLIB, LyricsError, Query, fetch, from_lrclib, pick_best,
    retry_after_secs,
};
pub use lrc::{clean_artist, clean_title, current_index, parse_lrc};

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
