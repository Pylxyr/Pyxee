//! Sample sources: symphonia (pure Rust) and an ffmpeg pipe (for codecs symphonia lacks, e.g. Opus).

use crate::AudioError;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
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

/// Decodes anything ffmpeg understands to 48 kHz stereo f32 over a pipe.
pub struct FfmpegPipeSource {
    child: Child,
    stdout: ChildStdout,
    leftover: Vec<u8>,
    last_error: Arc<Mutex<String>>,
    stderr_thread: Option<JoinHandle<()>>,
    checked_exit: bool,
}

pub const FFMPEG_RATE: u32 = 48_000;

impl FfmpegPipeSource {
    pub fn spawn(ffmpeg: &Path, input: &str) -> Result<Self, AudioError> {
        Self::spawn_at(ffmpeg, input, 0)
    }

    /// Starts decoding at `start_ms` (ffmpeg seeks before decoding).
    pub fn spawn_at(ffmpeg: &Path, input: &str, start_ms: u64) -> Result<Self, AudioError> {
        let mut cmd = Command::new(ffmpeg);
        cmd.args(["-nostdin", "-v", "error"]);
        // Only the protocols we need; reconnect on dropped HTTP streams.
        cmd.args(["-protocol_whitelist", "file,http,https,tcp,tls,crypto"]);
        if input.starts_with("http") {
            cmd.args([
                "-reconnect",
                "1",
                "-reconnect_streamed",
                "1",
                "-reconnect_delay_max",
                "5",
            ]);
        }
        if start_ms > 0 {
            cmd.args([
                "-ss",
                &format!("{}.{:03}", start_ms / 1000, start_ms % 1000),
            ]);
        }
        cmd.args(["-i", input, "-vn", "-f", "f32le", "-ac", "2", "-ar"])
            .arg(FFMPEG_RATE.to_string())
            .arg("pipe:1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().map_err(|e| {
            AudioError::Ffmpeg(format!(
                "ffmpeg could not be started ({}): {e}. Pyxee needs ffmpeg for most YouTube audio: run `cargo xtask tools` or set PYXEE_FFMPEG.",
                ffmpeg.display()
            ))
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AudioError::Ffmpeg("no stdout".into()))?;
        let last_error = Arc::new(Mutex::new(String::new()));
        let stderr_thread = child.stderr.take().map(|stderr| {
            let sink = Arc::clone(&last_error);
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if let Ok(mut g) = sink.lock() {
                        *g = line;
                    }
                }
            })
        });
        Ok(Self {
            child,
            stdout,
            leftover: Vec::new(),
            last_error,
            stderr_thread,
            checked_exit: false,
        })
    }

    /// At end of stream, a non-zero exit means ffmpeg failed (bad URL, unsupported file), not "track over".
    fn check_exit(&mut self) -> Result<(), AudioError> {
        if self.checked_exit {
            return Ok(());
        }
        self.checked_exit = true;
        let status = self.child.wait()?;
        if let Some(t) = self.stderr_thread.take() {
            let _ = t.join();
        }
        if !status.success() {
            let last = self
                .last_error
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default();
            log::warn!("ffmpeg exited with {status}: {last}");
        }
        if status.success() {
            Ok(())
        } else {
            let msg = self
                .last_error
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default();
            Err(AudioError::Ffmpeg(format!(
                "ffmpeg exited with {status}: {msg}"
            )))
        }
    }
}

impl Drop for FfmpegPipeSource {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl SampleSource for FfmpegPipeSource {
    fn sample_rate(&self) -> u32 {
        FFMPEG_RATE
    }
    fn channels(&self) -> usize {
        2
    }

    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError> {
        let mut raw = [0u8; 16 * 1024];
        let n = self.stdout.read(&mut raw)?;
        if n == 0 {
            self.check_exit()?;
            return Ok(false);
        }
        self.leftover.extend_from_slice(&raw[..n]);
        let usable = self.leftover.len() / 8 * 8; // whole stereo f32 frames
        out.extend(
            self.leftover[..usable]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b)),
        );
        self.leftover.drain(..usable);
        Ok(true)
    }
}
