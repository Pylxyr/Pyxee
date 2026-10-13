use crate::{clean_artist, clean_title, parse_lrc, Lyrics};
use serde::Deserialize;
use std::time::Duration;

pub const LRCLIB: &str = "https://lrclib.net";
const USER_AGENT: &str = "Pyxee/0.0.1 (https://github.com/Pylxyr/Pyxee)";
/// A match whose length differs by more than this is probably another recording.
const DURATION_SLACK_SECS: f64 = 4.0;

#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("could not reach the lyrics service")]
    Network,
    #[error("the lyrics service answered something unexpected: {0}")]
    Parse(String),
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
