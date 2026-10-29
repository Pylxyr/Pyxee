//! Sample sources. FFmpeg's libavformat/libavcodec (a minimal static LGPL build, linked in-process
//! through the C shim in `csrc/pxdec.c`) is the only decoder: no `ffmpeg` process is ever started.
//! A format the build cannot open fails with `AudioError::Unsupported`.

use crate::AudioError;
use std::ffi::{c_int, c_void, CStr};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

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

/// A seekable byte stream the decoder reads from: a file or an HTTP range reader.
pub trait ByteSource: Read + Seek + Send {
    /// Total length in bytes, if known.
    fn byte_len(&self) -> Option<u64>;
}

impl ByteSource for std::fs::File {
    fn byte_len(&self) -> Option<u64> {
        self.metadata().ok().map(|m| m.len())
    }
}

mod ffi {
    use std::ffi::{c_int, c_void};

    #[repr(C)]
    pub struct PxDec {
        _private: [u8; 0],
    }

    pub type ReadFn = unsafe extern "C" fn(*mut c_void, *mut u8, c_int) -> c_int;
    pub type SeekFn = unsafe extern "C" fn(*mut c_void, i64, c_int) -> i64;
    pub type SizeFn = unsafe extern "C" fn(*mut c_void) -> i64;

    extern "C" {
        pub fn px_open(
            opaque: *mut c_void,
            rd: ReadFn,
            sk: SeekFn,
            sz: SizeFn,
            err: *mut c_int,
            io_failed: *mut c_int,
        ) -> *mut PxDec;
        pub fn px_sample_rate(d: *const PxDec) -> c_int;
        pub fn px_is_aac(d: *const PxDec) -> c_int;
        pub fn px_read(
            d: *mut PxDec,
            samples: *mut *const f32,
            frames: *mut c_int,
            io_failed: *mut c_int,
        ) -> c_int;
        pub fn px_seek_ms(d: *mut PxDec, ms: i64) -> c_int;
        pub fn px_close(d: *mut PxDec);
        pub fn px_strerror(err: c_int, buf: *mut std::ffi::c_char, len: c_int);
        pub fn px_err_decoder_not_found() -> c_int;
        pub fn px_err_invaliddata() -> c_int;
    }
}

/// What the C callbacks read from, plus the I/O error that made them fail (the C side only sees
/// "failed"; the details, such as an HTTP status, are kept here to map to a precise error).
struct IoCtx {
    src: Box<dyn ByteSource>,
    err: Option<std::io::Error>,
}

/// The callbacks must never unwind into C: a panic is turned into a failed read.
fn guarded<T>(
    ctx: *mut c_void,
    fail: T,
    f: impl FnOnce(&mut IoCtx) -> Result<T, std::io::Error>,
) -> T {
    // SAFETY: `ctx` is the leaked `IoCtx` owned by the `LibavSource` that owns the decoder; libav only
    // calls back while that source is inside `px_open`/`px_read`/`px_seek_ms`, so it is live and
    // not aliased.
    let ctx = unsafe { &mut *(ctx as *mut IoCtx) };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(ctx))) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            ctx.err = Some(e);
            fail
        }
        Err(_) => {
            ctx.err = Some(std::io::Error::other("panic in the byte source"));
            fail
        }
    }
}

unsafe extern "C" fn read_cb(ctx: *mut c_void, buf: *mut u8, len: c_int) -> c_int {
    guarded(ctx, -1, |c| {
        // SAFETY: libav passes a writable buffer of `len` bytes.
        let out = unsafe { std::slice::from_raw_parts_mut(buf, len.max(0) as usize) };
        loop {
            match c.src.read(out) {
                Ok(n) => return Ok(n as c_int),
                Err(e)
                    if e.kind() == std::io::ErrorKind::Interrupted
                        && !e
                            .get_ref()
                            .is_some_and(|x| x.is::<crate::http::Cancelled>()) =>
                {
                    continue
                }
                Err(e) => return Err(e),
            }
        }
    })
}

unsafe extern "C" fn seek_cb(ctx: *mut c_void, offset: i64, whence: c_int) -> i64 {
    guarded(ctx, -1, |c| {
        let from = match whence {
            0 => SeekFrom::Start(u64::try_from(offset).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek before start")
            })?),
            1 => SeekFrom::Current(offset),
            2 => SeekFrom::End(offset),
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "bad seek mode",
                ))
            }
        };
        c.src.seek(from).map(|p| p as i64)
    })
}

unsafe extern "C" fn size_cb(ctx: *mut c_void) -> i64 {
    guarded(ctx, -1, |c| Ok(c.src.byte_len().map_or(-1, |n| n as i64)))
}

fn map_io_err(e: std::io::Error) -> AudioError {
    if let Some(h) = e
        .get_ref()
        .and_then(|x| x.downcast_ref::<crate::http::HttpStatus>())
    {
        return if crate::http::is_expired(h.0) {
            AudioError::UrlExpired(h.0)
        } else if h.0 == 429 {
            AudioError::RateLimited
        } else {
            AudioError::Network(format!("HTTP {}", h.0))
        };
    }
    AudioError::Io(e)
}

fn describe(code: c_int) -> String {
    let mut buf = [0 as std::ffi::c_char; 128];
    // SAFETY: `buf` is writable for its stated length and `px_strerror` NUL-terminates it.
    unsafe {
        ffi::px_strerror(code, buf.as_mut_ptr(), buf.len() as c_int);
        CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
    }
}

/// Max frames of encoder priming/padding trimmed from AAC when the container did not (typical LC
/// is 1024-2112). libav trims what the container declares; plain and YouTube-style fragmented M4A
/// declare none or only the start, so quiet edges are trimmed on top (a no-op once libav did it).
const EDGE_MAX_FRAMES: usize = 2112;
/// Anything below this is treated as the encoder's silent priming/padding.
const QUIET: f32 = 1e-3;

/// Decoded audio from any container the minimal FFmpeg build opens, as stereo `f32` at the
/// stream's own rate. libavformat/libavcodec apply the container's encoder delay and padding
/// (Opus pre-skip and end trim, MP3/AAC gapless data), so no edge heuristics are needed here.
pub struct LibavSource {
    dec: *mut ffi::PxDec,
    // Owned through a raw pointer (from `Box::into_raw`) because the C side holds the same address: a `Box` that is
    // moved while C keeps a pointer derived from it is a provenance violation. Freed in `Drop`, after `dec` is closed.
    ctx: std::ptr::NonNull<IoCtx>,
    rate: u32,
    /// Frames to drop from the output (used by `skip_ms` for streams that cannot seek in place).
    skip_frames: u64,
    finished: bool,
    /// AAC: trim quiet edges (capped), see [`EDGE_MAX_FRAMES`].
    edge_trim: bool,
    lead_pending: bool,
    lead_dropped: usize,
    tail: Vec<f32>,
    chunk: Vec<f32>,
}

// SAFETY: the libav contexts have no thread affinity and are only ever used through `&mut self`,
// so moving the owner to another thread is sound. The byte source inside is `Send`.
unsafe impl Send for LibavSource {}

impl LibavSource {
    pub fn open_path(path: &Path) -> Result<Self, AudioError> {
        Self::open(Box::new(std::fs::File::open(path)?))
    }

    pub fn open(source: Box<dyn ByteSource>) -> Result<Self, AudioError> {
        let ctx = std::ptr::NonNull::from(Box::leak(Box::new(IoCtx {
            src: source,
            err: None,
        })));
        let (mut err, mut io_failed) = (0 as c_int, 0 as c_int);
        let opaque = ctx.as_ptr().cast::<c_void>();
        // SAFETY: `opaque` points at the leaked `IoCtx`, which is freed only after the decoder is closed (see `Drop`
        // and the failure path below); the callbacks match the declared C signatures.
        let dec =
            unsafe { ffi::px_open(opaque, read_cb, seek_cb, size_cb, &mut err, &mut io_failed) };
        if dec.is_null() {
            // SAFETY: `px_open` failed and closed everything, so nothing else uses the context any more.
            let mut ctx = unsafe { Box::from_raw(ctx.as_ptr()) };
            if let Some(e) = ctx.err.take() {
                return Err(map_io_err(e));
            }
            // SAFETY: plain constant getters.
            let (no_dec, bad) =
                unsafe { (ffi::px_err_decoder_not_found(), ffi::px_err_invaliddata()) };
            let why = describe(err);
            return Err(if err == no_dec || err == bad || io_failed == 0 {
                AudioError::Unsupported(format!("cannot open this media: {why}"))
            } else {
                AudioError::Decode(why)
            });
        }
        // SAFETY: `dec` is a live decoder.
        let rate = unsafe { ffi::px_sample_rate(dec) };
        let rate = u32::try_from(rate)
            .map_err(|_| AudioError::Unsupported("unknown sample rate".into()))?;
        // SAFETY: `dec` is a live decoder.
        let edge_trim = unsafe { ffi::px_is_aac(dec) } != 0;
        Ok(Self {
            dec,
            ctx,
            rate,
            skip_frames: 0,
            finished: false,
            edge_trim,
            lead_pending: edge_trim,
            lead_dropped: 0,
            tail: Vec::new(),
            chunk: Vec::new(),
        })
    }

    /// Drops the first `ms` of audio by decoding through it. Slower than [`SampleSource::seek_ms`]
    /// but needs no seek support: the fallback for a stream that cannot seek in place. Call it on
    /// a freshly opened source (a failed seek may have moved the reader).
    pub fn skip_ms(&mut self, ms: u64) {
        self.skip_frames = ms * u64::from(self.rate) / 1000;
        self.lead_pending = false; // the quiet start is inside what is being dropped
    }

    /// The context the C callbacks use. Only reached through `&mut self`, and libav calls back only while one of this
    /// type's methods is inside a `px_*` call, so there is no other live reference.
    fn ctx_mut(&mut self) -> &mut IoCtx {
        // SAFETY: `ctx` came from `Box::into_raw`-style leaking in `open` and lives until `Drop`.
        unsafe { self.ctx.as_mut() }
    }

    // Note: a recorded I/O error is deliberately kept until it is reported. libav may swallow a failed read and report
    // a plain end of stream; the error recorded here is how a cut-off stream is told from a song that really ended
    // (`an_expired_stream_url_mid_song_is_re_resolved...` in the app's engine tests depends on it).
    fn fail(&mut self, code: c_int) -> AudioError {
        match self.ctx_mut().err.take() {
            Some(e) => map_io_err(e),
            None => AudioError::Decode(describe(code)),
        }
    }

    fn is_quiet(frame: &[f32]) -> bool {
        frame.iter().all(|s| s.abs() < QUIET)
    }

    /// Decodes the next chunk of stereo frames into `chunk` (cleared first). `Ok(false)` at the end.
    fn read_raw(&mut self, chunk: &mut Vec<f32>) -> Result<bool, AudioError> {
        chunk.clear();
        loop {
            let mut samples: *const f32 = std::ptr::null();
            let (mut frames, mut io_failed) = (0 as c_int, 0 as c_int);
            // SAFETY: `dec` is live; on a result of 1 `samples` points at `frames` interleaved
            // stereo frames that stay valid until the next call, and are copied out right away.
            let r = unsafe { ffi::px_read(self.dec, &mut samples, &mut frames, &mut io_failed) };
            if r < 0 {
                return Err(self.fail(r));
            }
            if r == 0 {
                return Ok(false);
            }
            let frames = usize::try_from(frames).unwrap_or(0);
            let drop = (self.skip_frames as usize).min(frames);
            self.skip_frames -= drop as u64;
            if drop == frames {
                continue;
            }
            // SAFETY: see above; `frames * CHANNELS` samples are readable.
            let all = unsafe { std::slice::from_raw_parts(samples, frames * crate::CHANNELS) };
            chunk.extend_from_slice(&all[drop * crate::CHANNELS..]);
            return Ok(true);
        }
    }
}

impl Drop for LibavSource {
    fn drop(&mut self) {
        // SAFETY: `dec` came from `px_open` and is closed exactly once, before `ctx` is freed; after the close
        // libav makes no more callbacks, so nothing uses `ctx`, which is the pointer `open` leaked.
        unsafe {
            ffi::px_close(self.dec);
            drop(Box::from_raw(self.ctx.as_ptr()));
        }
    }
}

impl SampleSource for LibavSource {
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn channels(&self) -> usize {
        crate::CHANNELS
    }

    fn seek_ms(&mut self, ms: u64) -> Result<(), AudioError> {
        let ms =
            i64::try_from(ms).map_err(|_| AudioError::Decode("seek position too large".into()))?;
        // SAFETY: `dec` is a live decoder.
        let r = unsafe { ffi::px_seek_ms(self.dec, ms) };
        if r < 0 {
            return Err(match self.ctx_mut().err.take() {
                Some(e) => map_io_err(e),
                None => AudioError::Decode(format!("seek failed: {}", describe(r))),
            });
        }
        self.skip_frames = 0;
        self.finished = false;
        // Priming/padding trimming only applies to the true start and end of a stream.
        self.lead_pending = false;
        self.tail.clear();
        Ok(())
    }

    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError> {
        if self.finished {
            return Ok(false);
        }
        const CH: usize = crate::CHANNELS;
        loop {
            let mut chunk = std::mem::take(&mut self.chunk);
            let more = self.read_raw(&mut chunk)?;
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
                let mut keep = self.tail.len() / CH;
                while keep > 0 && Self::is_quiet(&self.tail[(keep - 1) * CH..keep * CH]) {
                    keep -= 1;
                }
                out.extend_from_slice(&self.tail[..keep * CH]);
                self.tail.clear();
                self.chunk = chunk;
                return Ok(keep > 0);
            }
            // Leading edge: drop quiet frames (capped) until real audio starts.
            let mut start = 0;
            if self.lead_pending {
                let frames = chunk.len() / CH;
                while start < frames
                    && self.lead_dropped < EDGE_MAX_FRAMES
                    && Self::is_quiet(&chunk[start * CH..(start + 1) * CH])
                {
                    start += 1;
                    self.lead_dropped += 1;
                }
                if start < frames {
                    self.lead_pending = false;
                }
            }
            self.tail.extend_from_slice(&chunk[start * CH..]);
            self.chunk = chunk;
            // Hold back the last EDGE_MAX_FRAMES so padding can be trimmed at EOF.
            let hold = EDGE_MAX_FRAMES * CH;
            if self.tail.len() > hold {
                let emit = self.tail.len() - hold;
                out.extend_from_slice(&self.tail[..emit]);
                self.tail.drain(..emit);
                return Ok(true);
            }
        }
    }
}
