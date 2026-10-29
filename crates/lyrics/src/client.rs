use crate::{clean_artist, clean_title, parse_lrc, Lyrics};
use serde::Deserialize;
use std::time::Duration;

pub const LRCLIB: &str = "https://lrclib.net";
/// LRCLIB asks clients to say who they are, with a version.
const USER_AGENT: &str = concat!(
    "Pyxee/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Pylxyr/Pyxee)"
);
/// A match whose length differs by more than this is probably another recording.
const DURATION_SLACK_SECS: f64 = 4.0;

#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("could not reach the lyrics service")]
    Network,
    /// HTTP 429. `retry_after_secs` is the service's `Retry-After`, or a default, within sane bounds.
    #[error("the lyrics service is limiting requests, try again in about {retry_after_secs} s")]
    RateLimited { retry_after_secs: u64 },
    /// HTTP 401/403: the service refuses this client.
    #[error("the lyrics service refused the request (HTTP {0})")]
    Blocked(u16),
    /// HTTP 5xx: the service itself is failing.
    #[error("the lyrics service is having trouble (HTTP {0})")]
    Unavailable(u16),
    #[error("the lyrics service answered something unexpected: {0}")]
    Parse(String),
}

/// Waits when the service gave no `Retry-After`, and the bounds on one it gave.
const DEFAULT_RETRY_SECS: u64 = 60;
const MAX_RETRY_SECS: u64 = 600;
const BLOCKED_PAUSE_SECS: u64 = 300;
const UNAVAILABLE_PAUSE_SECS: u64 = 30;

/// Seconds to wait from a `Retry-After` header (the number form; the HTTP-date form is not worth
/// parsing here and gets the default), kept between 1 s and 10 min.
pub fn retry_after_secs(header: Option<&str>) -> u64 {
    header
        .and_then(|h| h.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_RETRY_SECS)
        .clamp(1, MAX_RETRY_SECS)
}

impl LyricsError {
    /// How long to stop asking the service after this error, so a blocked or failing service is not
    /// hit again on every song change. `None`: asking again right away is fine (offline, bad data).
    pub fn pause_secs(&self) -> Option<u64> {
        match self {
            LyricsError::RateLimited { retry_after_secs } => Some(*retry_after_secs),
            LyricsError::Blocked(_) => Some(BLOCKED_PAUSE_SECS),
            LyricsError::Unavailable(_) => Some(UNAVAILABLE_PAUSE_SECS),
            LyricsError::Network | LyricsError::Parse(_) => None,
        }
    }
}

/// Remembers "do not ask the service before this moment". The clock is injected (milliseconds from
/// any fixed start) so it is deterministic in tests.
#[derive(Debug, Default)]
pub struct Cooldown {
    until_ms: Option<u64>,
}

impl Cooldown {
    /// Whole seconds still to wait (at least 1), or `None` when the service may be asked.
    pub fn remaining_secs(&self, now_ms: u64) -> Option<u64> {
        self.until_ms
            .filter(|&t| t > now_ms)
            .map(|t| (t - now_ms).div_ceil(1000))
    }

    /// Notes a failed lookup; errors that carry no pause leave the cooldown as it was.
    pub fn record(&mut self, err: &LyricsError, now_ms: u64) {
        if let Some(secs) = err.pause_secs() {
            let until = now_ms.saturating_add(secs.saturating_mul(1000));
            self.until_ms = Some(self.until_ms.map_or(until, |t| t.max(until)));
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub title: String,
    pub artist: String,
    pub duration_secs: Option<f64>,
}

impl Query {
    /// Cleans what the player knows (titles like "Song (Official Video)", artists like "A, B") for the service.
    pub fn new(title: &str, artist: &str, duration_ms: Option<u64>) -> Self {
        Self {
            title: clean_title(title),
            artist: clean_artist(artist),
            duration_secs: duration_ms.map(|ms| ms as f64 / 1000.0),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(default)]
    pub plain_lyrics: Option<String>,
    #[serde(default)]
    pub synced_lyrics: Option<String>,
}

impl Candidate {
    fn into_lyrics(self) -> Lyrics {
        if let Some(s) = self.synced_lyrics.as_deref() {
            let lines = parse_lrc(s);
            if !lines.is_empty() {
                return Lyrics::Synced(lines);
            }
        }
        if let Some(p) = self.plain_lyrics.filter(|p| !p.trim().is_empty()) {
            return Lyrics::Plain(p.trim().to_string());
        }
        if self.instrumental {
            Lyrics::Instrumental
        } else {
            Lyrics::None
        }
    }
}

/// The candidate to show: close in length to the song, synced before plain, plain before nothing.
pub fn pick_best(mut list: Vec<Candidate>, duration_secs: Option<f64>) -> Option<Candidate> {
    let close = |c: &Candidate| match (duration_secs, c.duration) {
        (Some(want), Some(have)) => (want - have).abs() <= DURATION_SLACK_SECS,
        _ => true, // unknown length on either side: cannot rule it out
    };
    list.retain(close);
    let rank = |c: &Candidate| {
        let synced = c
            .synced_lyrics
            .as_deref()
            .is_some_and(|s| !parse_lrc(s).is_empty());
        let plain = c
            .plain_lyrics
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty());
        match (synced, plain, c.instrumental) {
            (true, _, _) => 3,
            (_, true, _) => 2,
            (_, _, true) => 1,
            _ => 0,
        }
    };
    list.into_iter()
        .max_by_key(|c| rank(c))
        .filter(|c| rank(c) > 0)
}

/// Lyrics from a service answer: one object (`/api/get`) or a list (`/api/search`).
pub fn from_lrclib(json: &str, duration_secs: Option<f64>) -> Result<Lyrics, LyricsError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| LyricsError::Parse(e.to_string()))?;
    let list: Vec<Candidate> = match value {
        serde_json::Value::Array(_) => serde_json::from_value(value),
        serde_json::Value::Object(_) => serde_json::from_value(value).map(|c| vec![c]),
        _ => return Err(LyricsError::Parse("not an object or a list".into())),
    }
    .map_err(|e| LyricsError::Parse(e.to_string()))?;
    Ok(pick_best(list, duration_secs).map_or(Lyrics::None, Candidate::into_lyrics))
}

fn get(agent: &ureq::Agent, url: &str) -> Result<Option<String>, LyricsError> {
    match agent.get(url).set("User-Agent", USER_AGENT).call() {
        Ok(r) => r.into_string().map(Some).map_err(|_| LyricsError::Network),
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(ureq::Error::Status(429, resp)) => Err(LyricsError::RateLimited {
            retry_after_secs: retry_after_secs(resp.header("Retry-After")),
        }),
        Err(ureq::Error::Status(code @ (401 | 403), _)) => Err(LyricsError::Blocked(code)),
        Err(ureq::Error::Status(code @ 500..=599, _)) => Err(LyricsError::Unavailable(code)),
        Err(ureq::Error::Status(code, _)) => Err(LyricsError::Parse(format!("HTTP {code}"))),
        Err(ureq::Error::Transport(_)) => Err(LyricsError::Network),
    }
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Looks the song up at `base` (`LRCLIB`, or a test server): exact match first, then a search.
pub fn fetch(base: &str, q: &Query) -> Result<Lyrics, LyricsError> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(6))
        .timeout_read(Duration::from_secs(10))
        .build();
    let (t, a) = (encode(&q.title), encode(&q.artist));
    let mut exact = format!("{base}/api/get?track_name={t}&artist_name={a}");
    if let Some(d) = q.duration_secs {
        exact.push_str(&format!("&duration={}", d.round() as u64));
    }
    if let Some(body) = get(&agent, &exact)? {
        let found = from_lrclib(&body, q.duration_secs)?;
        if found != Lyrics::None {
            return Ok(found);
        }
    }
    let search = format!("{base}/api/search?track_name={t}&artist_name={a}");
    match get(&agent, &search)? {
        Some(body) => from_lrclib(&body, q.duration_secs),
        None => Ok(Lyrics::None),
    }
}

#[cfg(test)]
mod user_agent {
    #[test]
    fn says_which_version_of_pyxee_is_asking() {
        assert!(super::USER_AGENT.starts_with(&format!("Pyxee/{}", env!("CARGO_PKG_VERSION"))));
        assert!(!super::USER_AGENT.contains("0.0.1"));
    }
}
