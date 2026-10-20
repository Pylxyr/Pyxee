//! Seekable HTTP source using range requests, so the decoder can probe and seek streamed audio.
//!
//! A background thread downloads the song ahead of the decoder in `CHUNK`-sized range requests, and the decoder only
//! ever reads from memory. Once the song is downloaded it plays to the end with no network at all, and a short outage
//! costs nothing while enough is buffered. If the buffer runs dry the reader waits (up to `STALL_LIMIT`) while the
//! download keeps retrying, so a connection that comes back resumes the song where it stopped.
//!
//! Songs up to `FULL_LIMIT` are kept whole. A huge file (a long mix) is only buffered in a window around the playhead.

use crate::AudioError;
use std::collections::BTreeMap;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

const CHUNK: u64 = 512 * 1024;
/// Files up to this size are downloaded completely; bigger ones only around the playhead.
const FULL_LIMIT: u64 = 64 * 1024 * 1024;
/// For files above `FULL_LIMIT`: chunks kept ahead of and behind the playhead.
const WINDOW_AHEAD: u64 = 64;
const WINDOW_BEHIND: u64 = 4;
/// How long a read waits for data that is not buffered and cannot be downloaded before the song fails.
const STALL_LIMIT: Duration = Duration::from_secs(45);
/// The longest pause between retries of a failed download.
const RETRY_PAUSE_MAX: Duration = Duration::from_secs(3);
/// A read waiting for data looks at its cancel flag at least this often.
const CANCEL_CHECK: Duration = Duration::from_millis(100);

#[derive(Default)]
struct State {
    chunks: BTreeMap<u64, Arc<Vec<u8>>>,
    /// The chunk the decoder is reading; downloading starts here and runs forward.
    cursor: u64,
    /// A 4xx answer (expired URL, rate limit): nothing more can be downloaded from this URL.
    fatal: Option<u16>,
    last_error: Option<String>,
    closed: bool,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub struct HttpRangeSource {
    shared: Arc<Shared>,
    len: u64,
    pos: u64,
    /// The chunk last read from, so most reads need no lock.
    current: Option<(u64, Arc<Vec<u8>>)>,
    /// Set by whoever needs a read that is waiting for data to give up (see `chunk_for`).
    cancel: Arc<AtomicBool>,
}

fn net(e: impl std::fmt::Display) -> AudioError {
    AudioError::Network(e.to_string())
}

/// HTTP status carried inside `io::Error` so the decoder layer can tell "URL expired" from other failures.
#[derive(Debug)]
pub(crate) struct HttpStatus(pub u16);

impl std::fmt::Display for HttpStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP {}", self.0)
    }
}

impl std::error::Error for HttpStatus {}

/// Carried inside the `Interrupted` error of a read that was cancelled, so the decoder layer can tell it
/// from an ordinary interruption (which it retries).
#[derive(Debug)]
pub(crate) struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the read was cancelled")
    }
}

impl std::error::Error for Cancelled {}

pub(crate) fn is_expired(code: u16) -> bool {
    matches!(code, 401 | 403 | 410)
}

/// Applies the headers a stream URL needs (User-Agent etc.). Range is set by the caller and never overridden.
fn with_headers(mut req: ureq::Request, headers: &[(String, String)]) -> ureq::Request {
    for (k, v) in headers {
        if k.eq_ignore_ascii_case("range") || k.eq_ignore_ascii_case("host") {
            continue;
        }
        req = req.set(k, v);
    }
    req
}
enum FetchError {
    /// A 4xx answer. Retrying cannot help.
    Fatal(u16),
    /// A timeout, dropped connection, 5xx or short answer. The network may come back.
    Transient(String),
}

struct Fetcher {
    agent: ureq::Agent,
    url: String,
    headers: Vec<(String, String)>,
    len: u64,
}

impl Fetcher {
    fn fetch(&self, idx: u64) -> Result<Vec<u8>, FetchError> {
        let from = idx * CHUNK;
        let to = (from + CHUNK).min(self.len) - 1;
        let want = to - from + 1;
        let resp = with_headers(self.agent.get(&self.url), &self.headers)
            .set("Range", &format!("bytes={from}-{to}"))
            .call();
        match resp {
            Ok(resp) if !answers_range(&resp, from) => Err(FetchError::Transient(
                "server ignored the range request".into(),
            )),
            Ok(resp) => {
                let mut data = Vec::with_capacity(want as usize);
                match resp.into_reader().take(want).read_to_end(&mut data) {
                    Ok(_) if data.len() as u64 == want => Ok(data),
                    Ok(_) => Err(FetchError::Transient(format!(
                        "short answer: {} of {want} bytes",
                        data.len()
                    ))),
                    Err(e) => Err(FetchError::Transient(e.to_string())),
                }
            }
            Err(ureq::Error::Status(c, _)) if (400..500).contains(&c) => Err(FetchError::Fatal(c)),
            Err(ureq::Error::Status(c, _)) => Err(FetchError::Transient(format!("HTTP {c}"))),
            Err(e) => Err(FetchError::Transient(e.to_string())),
        }
    }
}

/// A real answer to a range request: status 206 and a `Content-Range` that starts where the request did. A server that
/// answers 200 with the whole file (or a different part) must not have its bytes taken for the requested chunk.
fn answers_range(resp: &ureq::Response, from: u64) -> bool {
    resp.status() == 206
        && resp
            .header("Content-Range")
            .and_then(|v| v.trim().strip_prefix("bytes "))
            .and_then(|v| v.split('-').next())
            .and_then(|v| v.trim().parse::<u64>().ok())
            == Some(from)
}

/// The next chunk to download: the first missing one from the playhead forward (then any earlier gap, so the whole
/// song ends up in memory). For huge files only a window around the playhead, dropping chunks outside it.
fn next_wanted(st: &mut State, total: u64, big: bool) -> Option<u64> {
    let cur = st.cursor.min(total.saturating_sub(1));
    if big {
        let lo = cur.saturating_sub(WINDOW_BEHIND);
        let hi = (cur + WINDOW_AHEAD).min(total);
        st.chunks.retain(|i, _| *i >= lo && *i < hi);
        return (cur..hi).find(|i| !st.chunks.contains_key(i));
    }
    (cur..total)
        .chain(0..cur)
        .find(|i| !st.chunks.contains_key(i))
}

fn download_loop(shared: Arc<Shared>, f: Fetcher) {
    let total = f.len.div_ceil(CHUNK);
    let big = f.len > FULL_LIMIT;
    let mut failures: u32 = 0;
    loop {
        let idx = {
            let mut st = shared.lock();
            loop {
                if st.closed {
                    return;
                }
                match next_wanted(&mut st, total, big) {
                    Some(i) => break i,
                    None => {
                        st = shared.wake.wait(st).unwrap_or_else(PoisonError::into_inner);
                    }
                }
            }
        };
        match f.fetch(idx) {
            Ok(data) => {
                failures = 0;
                let mut st = shared.lock();
                st.last_error = None;
                st.chunks.insert(idx, Arc::new(data));
                shared.wake.notify_all();
            }
            Err(FetchError::Fatal(code)) => {
                shared.lock().fatal = Some(code);
                shared.wake.notify_all();
                return;
            }
            Err(FetchError::Transient(msg)) => {
                failures += 1;
                shared.lock().last_error = Some(msg);
                // Back off in short slices, so a closed source ends this thread promptly.
                let pause = Duration::from_millis(300 * u64::from(failures)).min(RETRY_PAUSE_MAX);
                let until = Instant::now() + pause;
                while Instant::now() < until {
                    if shared.lock().closed {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }
}

impl HttpRangeSource {
    pub fn open(url: &str) -> Result<Self, AudioError> {
        Self::open_with(url, &[])
    }

    /// Opens `url` sending `headers` on every request, and starts downloading it in the background.
    pub fn open_with(url: &str, headers: &[(String, String)]) -> Result<Self, AudioError> {
        Self::open_with_cancel(url, headers, Arc::new(AtomicBool::new(false)))
    }

    /// Like `open_with`, and a read that is waiting for data stops with `io::ErrorKind::Interrupted` while
    /// `cancel` is set (the pipeline sets it when it must flush, so a stalled stream cannot hold it up).
    pub fn open_with_cancel(
        url: &str,
        headers: &[(String, String)],
        cancel: Arc<AtomicBool>,
    ) -> Result<Self, AudioError> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(20))
            .build();
        let first = with_headers(agent.get(url), headers).set("Range", "bytes=0-0");
        let resp = match first.call() {
            Ok(r) => r,
            Err(ureq::Error::Status(c, _)) if is_expired(c) => {
                return Err(AudioError::UrlExpired(c))
            }
            Err(ureq::Error::Status(429, _)) => return Err(AudioError::RateLimited),
            Err(ureq::Error::Status(c, _)) => return Err(net(format!("HTTP {c}"))),
            Err(e) => return Err(net(e)),
        };
        let total = resp
            .header("Content-Range")
            .and_then(|v| v.rsplit('/').next())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .ok_or_else(|| {
                AudioError::Unsupported("the server does not support range requests".into())
            })?;
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
        });
        let fetcher = Fetcher {
            agent,
            url: url.to_string(),
            headers: headers.to_vec(),
            len: total,
        };
        let for_thread = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("pyxee-http-prefetch".into())
            .spawn(move || download_loop(for_thread, fetcher))
            .map_err(net)?;
        Ok(Self {
            shared,
            len: total,
            pos: 0,
            current: None,
            cancel,
        })
    }

    /// Bytes downloaded and held in memory so far.
    pub fn buffered_bytes(&self) -> u64 {
        self.shared
            .lock()
            .chunks
            .values()
            .map(|c| c.len() as u64)
            .sum()
    }

    /// The chunk `idx`, waiting for the download if it is not here yet.
    fn chunk_for(&mut self, idx: u64) -> io::Result<Arc<Vec<u8>>> {
        if let Some((i, data)) = &self.current {
            if *i == idx {
                return Ok(Arc::clone(data));
            }
        }
        let mut st = self.shared.lock();
        if st.cursor != idx {
            st.cursor = idx;
            self.shared.wake.notify_all();
        }
        let started = Instant::now();
        loop {
            if let Some(data) = st.chunks.get(&idx) {
                let data = Arc::clone(data);
                drop(st);
                self.current = Some((idx, Arc::clone(&data)));
                return Ok(data);
            }
            if let Some(code) = st.fatal {
                return Err(io::Error::other(HttpStatus(code)));
            }
            if self.cancel.load(Ordering::SeqCst) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, Cancelled));
            }
            if started.elapsed() >= STALL_LIMIT {
                let why = st
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "no data arrived".into());
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("the stream stalled for {}s: {why}", STALL_LIMIT.as_secs()),
                ));
            }
            let (guard, _) = self
                .shared
                .wake
                .wait_timeout(st, CANCEL_CHECK)
                .unwrap_or_else(PoisonError::into_inner);
            st = guard;
        }
    }
}

impl Drop for HttpRangeSource {
    fn drop(&mut self) {
        self.shared.lock().closed = true;
        self.shared.wake.notify_all();
    }
}

impl Read for HttpRangeSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len || out.is_empty() {
            return Ok(0);
        }
        let idx = self.pos / CHUNK;
        let data = self.chunk_for(idx)?;
        let off = (self.pos - idx * CHUNK) as usize;
        let n = out.len().min(data.len() - off);
        out[..n].copy_from_slice(&data[off..off + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for HttpRangeSource {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let new = match from {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::End(d) => self.len.checked_add_signed(d),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before start"))?;
        self.pos = new;
        Ok(new)
    }
}

impl crate::decode::ByteSource for HttpRangeSource {
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}
