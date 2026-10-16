//! Sample sources. Symphonia (pure Rust) is the only decoder: there is no ffmpeg. A format it cannot
//! open fails with `AudioError::Unsupported` (e.g. Opus: no released Symphonia has an Opus decoder).

use crate::AudioError;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

/// Anything that yields interleaved `f32` frames at a fixed rate/channel count.
pub trait SampleSource: Send {
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> usize;
    /// Appends decoded interleaved samples to `out`. `Ok(false)` means end of stream.
    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError>;
    /// Jumps to `ms` before further reads. Sources that cannot do it in place return an error
    /// and the caller re-opens them at the offset instead.
    fn seek_ms(&mut self, _ms: u64) -> Result<(), AudioError> {
        Err(AudioError::Unsupported(
            "this source cannot seek in place".into(),
        ))
    }
}

fn map_sym_err(e: SymError) -> AudioError {
    if let SymError::IoError(io) = &e {
        if let Some(h) = io
            .get_ref()
            .and_then(|x| x.downcast_ref::<crate::http::HttpStatus>())
        {
            return if crate::http::is_expired(h.0) {
                AudioError::UrlExpired(h.0)
            } else {
                AudioError::Network(format!("HTTP {}", h.0))
            };
        }
    }
    AudioError::Decode(e.to_string())
}

/// Max frames of encoder priming/padding we are willing to trim for AAC (typical LC is 1024-2112).
const EDGE_MAX_FRAMES: usize = 2112;
/// Anything below this is treated as the encoder's silent priming/padding.
const QUIET: f32 = 1e-3;

pub struct SymphoniaSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    rate: u32,
    channels: usize,
    buf: Option<SampleBuffer<f32>>,
    /// AAC in MP4: symphonia 0.5 exposes no delay/padding, so we trim quiet edges (capped).
    edge_trim: bool,
    lead_pending: bool,
    lead_dropped: usize,
    tail: Vec<f32>,
    chunk: Vec<f32>,
    finished: bool,
    /// Frames to drop after a seek (the demuxer lands on a packet boundary at or before the target).
    skip_frames: u64,
}

impl SymphoniaSource {
    pub fn open_path(path: &Path) -> Result<Self, AudioError> {
        let file = std::fs::File::open(path)?;
        let ext = path.extension().and_then(|e| e.to_str());
        Self::open(Box::new(file), ext)
    }

    pub fn open(source: Box<dyn MediaSource>, extension: Option<&str>) -> Result<Self, AudioError> {
        let mss = MediaSourceStream::new(source, Default::default());
        let mut hint = Hint::new();
        if let Some(e) = extension {
            hint.with_extension(e);
        }
        let fmt_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &MetadataOptions::default())
            .map_err(|e| match e {
                SymError::IoError(_) => map_sym_err(e),
                e => AudioError::Unsupported(format!("probe failed: {e}")),
            })?;
        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| AudioError::Unsupported("no audio track".into()))?;
        let params = track.codec_params.clone();
        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| AudioError::Unsupported(format!("no decoder for this codec: {e}")))?;
        let rate = params
            .sample_rate
            .ok_or_else(|| AudioError::Unsupported("unknown sample rate".into()))?;
        let channels = params.channels.map_or(2, |c| c.count());
        // MP3 and Vorbis trim themselves via symphonia's gapless support; AAC does not.
        let edge_trim =
            params.codec == symphonia::core::codecs::CODEC_TYPE_AAC && params.delay.is_none();
        Ok(Self {
            track_id: track.id,
            format,
            decoder,
            rate,
            channels,
            buf: None,
            edge_trim,
            lead_pending: edge_trim,
            lead_dropped: 0,
            tail: Vec::new(),
            chunk: Vec::new(),
            finished: false,
            skip_frames: 0,
        })
    }

    /// Drops the first `ms` of audio by decoding through it. Slower than [`SampleSource::seek_ms`]
    /// but needs no seek support: the fallback for a stream that cannot seek in place. Call it on
    /// a freshly opened source (a failed seek may have moved the reader).
    pub fn skip_ms(&mut self, ms: u64) {
        self.skip_frames = ms * u64::from(self.rate) / 1000;
        self.lead_pending = false; // the quiet start is inside what is being dropped
    }

    /// Decodes the next packet's samples, appending to `chunk`. `Ok(false)` at end of stream.
    fn decode_next(&mut self, chunk: &mut Vec<f32>) -> Result<bool, AudioError> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(false)
                }
                Err(SymError::ResetRequired) => return Ok(false),
                Err(e) => return Err(map_sym_err(e)),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let buf = self
                        .buf
                        .get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, spec));
                    if (buf.capacity() as u64)
                        < decoded.capacity() as u64 * spec.channels.count() as u64
                    {
                        *buf = SampleBuffer::new(decoded.capacity() as u64, spec);
                    }
                    buf.copy_interleaved_ref(decoded);
                    self.channels = spec.channels.count();
                    let samples = buf.samples();
                    let ch = self.channels.max(1);
                    let drop = (self.skip_frames as usize).min(samples.len() / ch);
                    self.skip_frames -= drop as u64;
                    let samples = &samples[drop * ch..];
                    if samples.is_empty() {
                        continue;
                    }
                    chunk.extend_from_slice(samples);
                    return Ok(true);
                }
                Err(SymError::DecodeError(_)) => continue, // skip a corrupt packet
                Err(e) => return Err(map_sym_err(e)),
            }
        }
    }

    fn is_quiet(frame: &[f32]) -> bool {
        frame.iter().all(|s| s.abs() < QUIET)
    }
}

impl SampleSource for SymphoniaSource {
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn channels(&self) -> usize {
        self.channels
    }

    fn seek_ms(&mut self, ms: u64) -> Result<(), AudioError> {
        let time = Time::new(ms / 1000, (ms % 1000) as f64 / 1000.0);
        let to = SeekTo::Time {
            time,
            track_id: Some(self.track_id),
        };
        let seeked = self
            .format
            .seek(SeekMode::Accurate, to)
            .map_err(|e| AudioError::Decode(format!("seek failed: {e}")))?;
        self.decoder.reset();
        self.skip_frames = seeked.required_ts.saturating_sub(seeked.actual_ts);
        // Priming/padding trimming only applies to the true start and end of a stream.
        self.lead_pending = false;
        self.tail.clear();
        self.finished = false;
        Ok(())
    }

    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError> {
        if self.finished {
            return Ok(false);
        }
        let ch = self.channels;
        loop {
            let mut chunk = std::mem::take(&mut self.chunk);
            chunk.clear();
            let more = self.decode_next(&mut chunk)?;
            let ch = if more { self.channels } else { ch };
            if !self.edge_trim {
                if more {
                    out.extend_from_slice(&chunk);
                } else {
                    self.finished = true;
                }
                self.chunk = chunk;
                return Ok(more);
            }
            if !more {
                // End of stream: drop quiet padding from the held-back tail, emit the rest.
                self.finished = true;
                let mut keep = self.tail.len() / ch;
                while keep > 0 && Self::is_quiet(&self.tail[(keep - 1) * ch..keep * ch]) {
                    keep -= 1;
                }
                out.extend_from_slice(&self.tail[..keep * ch]);
                self.tail.clear();
                self.chunk = chunk;
                return Ok(keep > 0);
            }
            // Leading edge: drop quiet frames (capped) until real audio starts.
            let mut start = 0;
            if self.lead_pending {
                let frames = chunk.len() / ch;
                while start < frames
                    && self.lead_dropped < EDGE_MAX_FRAMES
                    && Self::is_quiet(&chunk[start * ch..(start + 1) * ch])
                {
                    start += 1;
                    self.lead_dropped += 1;
                }
                if start < frames {
                    self.lead_pending = false;
                }
            }
            self.tail.extend_from_slice(&chunk[start * ch..]);
            self.chunk = chunk;
            // Hold back the last EDGE_MAX_FRAMES so padding can be trimmed at EOF.
            let hold = EDGE_MAX_FRAMES * ch;
            if self.tail.len() > hold {
                let emit = self.tail.len() - hold;
                out.extend_from_slice(&self.tail[..emit]);
                self.tail.drain(..emit);
                return Ok(true);
            }
        }
    }
}
