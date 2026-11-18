//! Pyxee audio engine pieces that need no audio hardware: decoding, resampling,
//! gapless joining and gap detection. Output devices live in the app/spike crates.

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

pub mod decode;
pub mod gap;
pub mod gapless;
pub mod http;
pub mod output;
pub mod pipeline;
pub mod resample;

use thiserror::Error;

/// Output format used by the engine: interleaved stereo `f32` at a device rate.
pub const CHANNELS: usize = 2;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported or undecodable media: {0}")]
    Unsupported(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("resampler error: {0}")]
    Resample(String),
    #[error("network error: {0}")]
    Network(String),
    /// HTTP 429 from the stream server: transient, so the player stays on the track instead of skipping.
    #[error("the stream server is limiting requests (HTTP 429)")]
    RateLimited,
    #[error("stream URL expired or was rejected (HTTP {0}); re-resolve the track")]
    UrlExpired(u16),
}
