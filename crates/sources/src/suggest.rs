//! Live search suggestions from Google's public suggest endpoint (the one YouTube's search box uses).

use crate::gate::escalate;
use crate::SourceError;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// How long an answer is reused (backspacing or retyping a prefix asks nothing).
const CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const CACHE_MAX: usize = 64;
/// After a rate-limit answer no suggestions are asked for this long; each one in a row doubles it, up to `PAUSE_MAX`.
const PAUSE_BASE: Duration = Duration::from_secs(60);
const PAUSE_MAX: Duration = Duration::from_secs(15 * 60);

pub const ENDPOINT: &str = "https://suggestqueries.google.com/complete/search";

/// Percent-encodes everything except unreserved characters.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Response shape: `["query", ["suggestion 1", "suggestion 2", ...]]`.
pub fn parse(body: &str) -> Result<Vec<String>, SourceError> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| SourceError::Parse(e.to_string()))?;
    let list = v
        .get(1)
        .and_then(|x| x.as_array())
        .ok_or_else(|| SourceError::Parse("no suggestion list".into()))?;
    let mut out: Vec<String> = Vec::new();
    for s in list.iter().filter_map(|s| s.as_str()) {
        if out.len() == 8 {
            break;
        }
        if !out.iter().any(|x| x == s) {
            out.push(s.to_string());
        }
    }
    Ok(out)
}

/// One request for the suggestions of `prefix`, with no caching and no pause. Prefer [`Suggester`].
pub fn fetch(prefix: &str) -> Result<Vec<String>, SourceError> {
    fetch_from(ENDPOINT, prefix)
}

fn fetch_from(endpoint: &str, prefix: &str) -> Result<Vec<String>, SourceError> {
    if prefix.trim().is_empty() {
        return Ok(Vec::new());
    }
    let url = format!(
        "{endpoint}?client=firefox&ds=yt&q={}",
        encode(prefix.trim())
    );
    match crate::net::agent()
        .get(&url)
        .timeout(Duration::from_secs(4))
        .call()
    {
        Ok(r) => parse(
            &r.into_string()
                .map_err(|e| SourceError::Failed(e.to_string()))?,
        ),
        Err(ureq::Error::Status(429 | 403, _)) => Err(SourceError::RateLimited),
        Err(ureq::Error::Status(c, _)) => Err(SourceError::Failed(format!(
            "suggestions failed (HTTP {c})"
        ))),
        Err(_) => Err(SourceError::Offline),
    }
}

#[derive(Default)]
struct State {
    /// Answers by lower-cased prefix, with the time they were stored.
    cache: HashMap<String, (Instant, Vec<String>)>,
    /// Rate limits in a row, with no answer in between. Sets how long the next pause is.
    strikes: u32,
    paused_until: Option<Instant>,
}

impl State {
    fn cached(&self, key: &str, now: Instant) -> Option<Vec<String>> {
        self.cache
            .get(key)
            .filter(|(at, _)| now.saturating_duration_since(*at) < CACHE_TTL)
            .map(|(_, list)| list.clone())
    }

    /// Time left on the pause after a rate limit, if one is on.
    fn paused(&self, now: Instant) -> Option<Duration> {
        self.paused_until
            .and_then(|t| t.checked_duration_since(now))
            .filter(|d| !d.is_zero())
    }

    fn store(&mut self, key: String, list: Vec<String>, now: Instant) {
        self.cache
            .retain(|_, (at, _)| now.saturating_duration_since(*at) < CACHE_TTL);
        if self.cache.len() >= CACHE_MAX && !self.cache.contains_key(&key) {
            if let Some(oldest) = self
                .cache
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(k, _)| k.clone())
            {
                self.cache.remove(&oldest);
            }
        }
        self.cache.insert(key, (now, list));
    }

    /// A rate limit came back: start (or lengthen) the pause and return how long it is.
    fn rate_limited(&mut self, now: Instant) -> Duration {
        let pause = escalate(PAUSE_BASE, PAUSE_MAX, self.strikes);
        self.strikes = self.strikes.saturating_add(1);
        self.paused_until = Some(now + pause);
        pause
    }

    /// Google answered: the pause (and its growth) starts over.
    fn answered(&mut self) {
        self.strikes = 0;
        self.paused_until = None;
    }
}

/// Suggestions for the search box, kept gentle on Google: a prefix asked before is answered from memory, and after a
/// rate-limit answer nothing is asked for a while (1 minute, doubling each time it happens again, up to 15). During
/// the pause `get` answers `RateLimited` at once; the app shows no suggestions then, which is fine while typing.
/// These requests go to a different host than yt-dlp's, so they are not part of the yt-dlp gate.
pub struct Suggester {
    endpoint: String,
    state: Mutex<State>,
}

impl Default for Suggester {
    fn default() -> Self {
        Self::new()
    }
}

impl Suggester {
    pub fn new() -> Self {
        Self::with_endpoint(ENDPOINT)
    }

    /// Asks `endpoint` instead of Google (for tests).
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            state: Mutex::new(State::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn get(&self, prefix: &str) -> Result<Vec<String>, SourceError> {
        let q = prefix.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let key = q.to_lowercase();
        {
            let st = self.lock();
            let now = Instant::now();
            if let Some(hit) = st.cached(&key, now) {
                return Ok(hit);
            }
            if let Some(left) = st.paused(now) {
                log::debug!("suggestions paused after a rate limit, {left:?} left");
                return Err(SourceError::RateLimited);
            }
        }
        // The lock is not held during the request.
        match fetch_from(&self.endpoint, q) {
            Ok(list) => {
                let mut st = self.lock();
                st.answered();
                st.store(key, list.clone(), Instant::now());
                Ok(list)
            }
            Err(SourceError::RateLimited) => {
                let pause = self.lock().rate_limited(Instant::now());
                // The typed text is not logged.
                log::warn!("search suggestions were rate limited: paused for {pause:?}");
                Err(SourceError::RateLimited)
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_spaces_unicode_and_reserved_characters() {
        assert_eq!(encode("lowtide neon"), "lowtide%20neon");
        assert_eq!(encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(encode("é"), "%C3%A9");
        assert_eq!(encode("safe-_.~"), "safe-_.~");
    }

    #[test]
    fn parses_the_suggest_response() {
        let body = r#"["lowt",["lowtide","lowtide neon harbor","lowtide live"]]"#;
        assert_eq!(
            parse(body).unwrap(),
            ["lowtide", "lowtide neon harbor", "lowtide live"]
        );
    }

    #[test]
    fn caps_the_number_of_suggestions_and_rejects_garbage() {
        let many = format!(
            r#"["q",[{}]]"#,
            (0..20)
                .map(|i| format!("\"s{i}\""))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(parse(&many).unwrap().len(), 8);
        assert!(matches!(parse("<html>"), Err(SourceError::Parse(_))));
        assert!(matches!(parse(r#"["q"]"#), Err(SourceError::Parse(_))));
    }

    #[test]
    fn a_suggestion_the_answer_repeats_is_listed_once() {
        // The search box keys its list by the text, so a repeat would be two rows with one key.
        let body = r#"["q",["a","b","a","c","b"]]"#;
        assert_eq!(parse(body).unwrap(), ["a", "b", "c"]);
    }

    #[test]
    fn blank_input_makes_no_request() {
        assert_eq!(fetch("   ").unwrap(), Vec::<String>::new());
    }

    // ---- Suggester ------------------------------------------------------------------------------

    use crate::ytmusic::testserver;

    const ANSWER: &str = r#"["lowt",["lowtide","lowtide live"]]"#;

    #[test]
    fn the_pause_after_a_rate_limit_doubles_and_an_answer_ends_it() {
        let t0 = Instant::now();
        let mut st = State::default();
        assert_eq!(st.paused(t0), None);
        assert_eq!(st.rate_limited(t0), Duration::from_secs(60));
        assert_eq!(
            st.paused(t0 + Duration::from_secs(10)),
            Some(Duration::from_secs(50))
        );
        assert_eq!(st.rate_limited(t0), Duration::from_secs(120));
        assert_eq!(
            st.paused(t0 + Duration::from_secs(61)),
            Some(Duration::from_secs(59))
        );
        for _ in 0..10 {
            st.rate_limited(t0);
        }
        assert_eq!(st.rate_limited(t0), PAUSE_MAX, "capped");
        st.answered();
        assert_eq!(st.paused(t0), None);
        assert_eq!(st.rate_limited(t0), PAUSE_BASE, "starts over");
    }

    #[test]
    fn cached_answers_expire_and_the_cache_stays_small() {
        let t0 = Instant::now();
        let mut st = State::default();
        st.store("a".into(), vec!["x".into()], t0);
        assert_eq!(
            st.cached("a", t0 + CACHE_TTL - Duration::from_secs(1)),
            Some(vec!["x".to_string()])
        );
        assert_eq!(st.cached("a", t0 + CACHE_TTL), None);
        for i in 0..(CACHE_MAX + 20) {
            st.store(
                format!("k{i}"),
                Vec::new(),
                t0 + Duration::from_millis(i as u64),
            );
        }
        assert_eq!(st.cache.len(), CACHE_MAX);
        assert!(
            st.cache.contains_key(&format!("k{}", CACHE_MAX + 19)),
            "the newest is kept"
        );
        assert!(!st.cache.contains_key("k0"), "the oldest went first");
    }

    #[test]
    fn a_prefix_asked_twice_is_one_request_whatever_its_case_or_spaces() {
        // The test server answers one request, then is gone: a second request could only fail.
        let (url, rx) = testserver::once(200, ANSWER);
        let s = Suggester::with_endpoint(url);
        assert_eq!(s.get("  LowT ").unwrap(), ["lowtide", "lowtide live"]);
        let sent = rx.recv().unwrap();
        assert!(
            sent.starts_with("GET /youtubei/v1/search?client=firefox&ds=yt&q=LowT "),
            "{sent}"
        );
        assert_eq!(s.get("lowt").unwrap(), ["lowtide", "lowtide live"]);
        assert_eq!(s.get("LOWT").unwrap(), ["lowtide", "lowtide live"]);
    }

    #[test]
    fn after_a_rate_limit_no_request_is_made_until_the_pause_is_over() {
        let (url, rx) = testserver::once(429, "slow down");
        let s = Suggester::with_endpoint(url);
        assert_eq!(s.get("lo"), Err(SourceError::RateLimited));
        rx.recv().unwrap();
        // The server is gone now: were a request made, the answer would be `Offline`, not `RateLimited`.
        assert_eq!(s.get("low"), Err(SourceError::RateLimited));
        assert_eq!(s.get("lowt"), Err(SourceError::RateLimited));
        assert_eq!(s.lock().strikes, 1, "refusals are not new strikes");
    }

    #[test]
    fn other_failures_neither_pause_nor_get_cached() {
        let (url, rx) = testserver::once(500, "oops");
        let s = Suggester::with_endpoint(url);
        assert!(matches!(s.get("lo"), Err(SourceError::Failed(_))));
        rx.recv().unwrap();
        assert_eq!(
            s.get("lo"),
            Err(SourceError::Offline),
            "asked again, not paused or cached"
        );
        assert_eq!(s.lock().strikes, 0);
    }

    #[test]
    fn blank_input_is_answered_without_a_request_or_a_cache_entry() {
        let s = Suggester::with_endpoint("http://127.0.0.1:1/never");
        assert_eq!(s.get("  ").unwrap(), Vec::<String>::new());
        assert!(s.lock().cache.is_empty());
    }
}
