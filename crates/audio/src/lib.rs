//! Pyxee audio engine pieces that need no audio hardware: decoding, resampling,
//! gapless joining and gap detection. Output devices live in the app/spike crates.

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
    #[error("stream URL expired or was rejected (HTTP {0}); re-resolve the track")]
    UrlExpired(u16),
}
