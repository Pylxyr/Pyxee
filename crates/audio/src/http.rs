//! Seekable HTTP source using range requests, so symphonia can probe and seek streamed audio.

use crate::AudioError;
use std::io::{self, Read, Seek, SeekFrom};
use std::time::Duration;
use symphonia::core::io::MediaSource;

const CHUNK: u64 = 512 * 1024;

pub struct HttpRangeSource {
    agent: ureq::Agent,
    url: String,
    len: u64,
    pos: u64,
    buf: Vec<u8>,
    buf_start: u64,
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

pub(crate) fn is_expired(code: u16) -> bool {
    matches!(code, 401 | 403 | 410)
}

impl HttpRangeSource {
    pub fn open(url: &str) -> Result<Self, AudioError> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(20))
            .build();
        let resp = match agent.get(url).set("Range", "bytes=0-0").call() {
            Ok(r) => r,
            Err(ureq::Error::Status(c, _)) if is_expired(c) => {
                return Err(AudioError::UrlExpired(c))
            }
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
        Ok(Self {
            agent,
            url: url.to_string(),
            len: total,
            pos: 0,
            buf: Vec::new(),
            buf_start: 0,
        })
    }

    /// Fetches one chunk, retrying transient failures (5xx, timeouts) twice. 4xx is never retried.
    fn fetch(&mut self, from: u64) -> io::Result<()> {
        let to = (from + CHUNK).min(self.len) - 1;
        let mut last = io::Error::other("no attempt made");
        for attempt in 0..3u64 {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(300 * attempt));
            }
            let resp = self
                .agent
                .get(&self.url)
                .set("Range", &format!("bytes={from}-{to}"))
                .call();
            match resp {
                Ok(resp) => {
                    let mut data = Vec::with_capacity((to - from + 1) as usize);
                    match resp
                        .into_reader()
                        .take(to - from + 1)
                        .read_to_end(&mut data)
                    {
                        Ok(_) => {
                            self.buf = data;
                            self.buf_start = from;
                            return Ok(());
                        }
                        Err(e) => last = e,
                    }
                }
                Err(ureq::Error::Status(c, _)) if (400..500).contains(&c) => {
                    return Err(io::Error::other(HttpStatus(c)));
                }
                Err(ureq::Error::Status(c, _)) => last = io::Error::other(HttpStatus(c)),
                Err(e) => last = io::Error::other(e.to_string()),
            }
        }
        Err(last)
    }
}

impl Read for HttpRangeSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len || out.is_empty() {
            return Ok(0);
        }
        let in_buf =
            self.pos >= self.buf_start && self.pos < self.buf_start + self.buf.len() as u64;
        if !in_buf {
            self.fetch(self.pos)?;
        }
        let off = (self.pos - self.buf_start) as usize;
        let n = out.len().min(self.buf.len() - off);
        out[..n].copy_from_slice(&self.buf[off..off + n]);
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

impl MediaSource for HttpRangeSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}
