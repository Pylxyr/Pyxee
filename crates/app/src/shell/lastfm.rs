//! Last.fm scrobbling through Last.fm's official web API (<https://www.last.fm/api>): "now playing" when a song
//! starts, and a scrobble once it has been listened to long enough (half of it or four minutes, whichever comes
//! first; never a song of 30 seconds or less).
//!
//! Like Discord presence, everything that decides something is a pure function or struct (`Tracker`, `sign`,
//! `parse_credential`, `parse_response` ...) and the only part that talks to the network is the `pyxee-lastfm`
//! thread. `on_state` runs on whatever thread publishes player state, so it never touches the network: it only
//! hands small messages to that thread, which also keeps the scrobbles Last.fm could not take yet and retries them
//! with a bounded back-off.
//!
//! The application's key and shared secret are built into the release (`PYXEE_LASTFM_API_KEY` and
//! `PYXEE_LASTFM_API_SECRET` when it was built; there is no setting or variable for users), like the Discord
//! application ID. The user signs in with Last.fm's desktop flow: the browser opens, they allow Pyxee, and Last.fm
//! answers with a session key that does not expire. Only that key is kept (in `shell.json`); the password never
//! reaches Pyxee. Neither the log nor any message ever contains a key, the secret, a token or a signature.

use crate::dto::TrackDto;
use md5::{Digest, Md5};
use pyxee_core::UNKNOWN_ARTIST;
use pyxee_shell::progress::Phase;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const API_URL: &str = "https://ws.audioscrobbler.com/2.0/";
const AUTH_URL: &str = "https://www.last.fm/api/auth/";

/// Last.fm only counts songs longer than this.
const MIN_SONG_MS: u64 = 30_000;
/// A song counts once half of it was heard, or this much of it, whichever comes first.
const MAX_LISTEN_MS: u64 = 4 * 60 * 1000;
/// A position that moved on by more than this between two states is a seek (or a stall), not listening.
const MAX_STEP_MS: u64 = 5_000;
/// A position back within the first seconds, from at least this far in, is the song starting over (repeat one, or
/// a seek to the start), which is a new listen.
const RESTART_WITHIN_MS: u64 = 5_000;
const RESTART_FROM_MS: u64 = 10_000;
/// Last.fm takes at most 50 scrobbles in one request.
const BATCH_MAX: usize = 50;
/// Scrobbles kept while Last.fm cannot be reached; the oldest go first.
const PENDING_MAX: usize = 500;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const BACKOFF_MIN: Duration = Duration::from_secs(5);
const BACKOFF_MAX: Duration = Duration::from_secs(5 * 60);
/// How often Last.fm is asked whether the user has allowed Pyxee yet, and for how long.
const AUTH_POLL: Duration = Duration::from_secs(3);
const AUTH_WINDOW: Duration = Duration::from_secs(10 * 60);
/// Asking for the request token is tried this many times (a second apart) before signing in is given up.
const TOKEN_TRIES: u32 = 3;
const TOKEN_RETRY_WAIT: Duration = Duration::from_secs(1);
/// How long quitting waits for the last scrobbles to be sent before giving up on them.
const SHUTDOWN_WAIT: Duration = Duration::from_millis(2_000);

// ---------------------------------------------------------------------------------------------------------------
// Pure decisions
// ---------------------------------------------------------------------------------------------------------------

/// A Last.fm API key or shared secret is 32 hexadecimal characters. Anything else is not one.
pub fn parse_credential(raw: &str) -> Option<String> {
    let v = raw.trim();
    (v.len() == 32 && v.bytes().all(|b| b.is_ascii_hexdigit())).then(|| v.to_owned())
}

/// The application's credentials. No `Debug`, so nothing can print them by accident.
#[derive(Clone)]
struct App {
    key: String,
    secret: String,
}

/// The credentials built into this release. A build without them has scrobbling unavailable; one with only one of
/// them, or an invalid one, is an error. The error never contains the values.
fn resolve_app(key: Option<&str>, secret: Option<&str>) -> Result<Option<App>, String> {
    fn given(v: Option<&str>) -> Option<&str> {
        v.filter(|s| !s.trim().is_empty())
    }
    match (given(key), given(secret)) {
        (None, None) => Ok(None),
        (Some(k), Some(s)) => match (parse_credential(k), parse_credential(s)) {
            (Some(key), Some(secret)) => Ok(Some(App { key, secret })),
            _ => Err(
                "the Last.fm application key or secret built into this release is not valid (32 hexadecimal characters each)"
                    .to_string(),
            ),
        },
        _ => Err("this release has only one of the Last.fm application key and secret".to_string()),
    }
}

/// Last.fm's request signature: the parameters sorted by name, each written as its name followed by its value,
/// then the shared secret, all hashed with MD5. `format` and `callback` are not part of it.
fn sign(params: &[(String, String)], secret: &str) -> String {
    let mut sorted: Vec<&(String, String)> = params
        .iter()
        .filter(|(k, _)| k != "format" && k != "callback")
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hash = Md5::new();
    for (k, v) in sorted {
        hash.update(k.as_bytes());
        hash.update(v.as_bytes());
    }
    hash.update(secret.as_bytes());
    format!("{:x}", hash.finalize())
}

/// What Last.fm is told about a song.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Song {
    pub title: String,
    pub artist: String,
    pub duration_ms: Option<u64>,
}

/// A song that was listened to, with the unix time (seconds) it started playing.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Scrobble {
    song: Song,
    timestamp: i64,
}

/// The song for a track, or `None` when Last.fm should not hear about it: no title, no known artist (it would
/// become an "Unknown artist" in the listener's history), or a length of 30 seconds or less.
fn song_of(t: &TrackDto) -> Option<Song> {
    let title = t.title.trim();
    let artist = t.artist.trim();
    if title.is_empty() || artist.is_empty() || artist.eq_ignore_ascii_case(UNKNOWN_ARTIST) {
        return None;
    }
    let duration_ms = t.duration_ms.filter(|d| *d > 0);
    if duration_ms.is_some_and(|d| d <= MIN_SONG_MS) {
        return None;
    }
    Some(Song {
        title: title.to_owned(),
        artist: artist.to_owned(),
        duration_ms,
    })
}

/// How much of the song has to be heard before it is scrobbled.
fn threshold_ms(song: &Song) -> u64 {
    song.duration_ms
        .map_or(MAX_LISTEN_MS, |d| (d / 2).min(MAX_LISTEN_MS))
}

/// What the tracker decides the worker should send.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    NowPlaying(Song),
    Scrobble(Scrobble),
}

/// One listen in progress.
struct Current {
    id: u64,
    /// Unix seconds when it started playing.
    started: i64,
    listened_ms: u64,
    last_pos: u64,
    announced: bool,
    scrobbled: bool,
}

/// Turns the player states `shell::on_state` receives into "now playing" and scrobbles. Time heard only counts
/// while the song is audible and the position moves on normally, so pausing, buffering and seeking add nothing.
#[derive(Default)]
struct Tracker {
    current: Option<Current>,
}

impl Tracker {
    /// `id` is the queue entry that is current (so the same song twice in a row is two listens), `now` is unix
    /// seconds.
    fn observe(
        &mut self,
        phase: Phase,
        id: Option<u64>,
        track: Option<&TrackDto>,
        position_ms: u64,
        now: i64,
    ) -> Vec<Action> {
        let (Some(id), Some(track), true) = (
            id,
            track,
            matches!(phase, Phase::Loading | Phase::Playing | Phase::Paused),
        ) else {
            self.current = None;
            return Vec::new();
        };
        if phase == Phase::Loading {
            // Resolving or buffering says nothing about listening; only another song ends the listen in progress.
            if self.current.as_ref().is_some_and(|c| c.id != id) {
                self.current = None;
            }
            return Vec::new();
        }
        let same_listen = self.current.as_ref().is_some_and(|c| {
            c.id == id && !(c.last_pos >= RESTART_FROM_MS && position_ms <= RESTART_WITHIN_MS)
        });
        if !same_listen {
            self.current = None;
        }
        let playing = phase == Phase::Playing;
        if self.current.is_none() {
            if !playing {
                return Vec::new(); // a listen starts when the song becomes audible
            }
            self.current = Some(Current {
                id,
                started: now.saturating_sub(i64::try_from(position_ms / 1000).unwrap_or(0)),
                listened_ms: 0,
                last_pos: position_ms,
                announced: false,
                scrobbled: false,
            });
        }
        let Some(cur) = self.current.as_mut() else {
            return Vec::new();
        };
        if playing && position_ms > cur.last_pos && position_ms - cur.last_pos <= MAX_STEP_MS {
            cur.listened_ms += position_ms - cur.last_pos;
        }
        cur.last_pos = position_ms;

        let mut out = Vec::new();
        if let Some(song) = song_of(track) {
            if playing && !cur.announced {
                cur.announced = true;
                out.push(Action::NowPlaying(song.clone()));
            }
            if !cur.scrobbled && cur.listened_ms >= threshold_ms(&song) {
                cur.scrobbled = true;
                out.push(Action::Scrobble(Scrobble {
                    song,
                    timestamp: cur.started,
                }));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Talking to Last.fm
// ---------------------------------------------------------------------------------------------------------------

/// Why a request did not work.
#[derive(Debug, PartialEq, Eq)]
enum ApiError {
    /// Last.fm answered with an error code (<https://www.last.fm/api/errorcodes>).
    Api { code: u32, message: String },
    /// Last.fm could not be reached, or answered with a server error.
    Network(String),
    /// The answer was not what the API documents.
    Parse(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Api { code, message } => write!(f, "Last.fm error {code}: {message}"),
            Self::Network(why) => write!(f, "Last.fm could not be reached ({why})"),
            Self::Parse(why) => write!(f, "unexpected answer from Last.fm ({why})"),
        }
    }
}

/// What to do about a failed request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Probably temporary: try again later.
    Retry,
    /// Last.fm no longer accepts the saved session: the user has to sign in again.
    SessionLost,
    /// Trying again cannot help (a wrong or suspended application key, a bad signature, ...).
    Fatal,
}

impl ApiError {
    fn outcome(&self) -> Outcome {
        match self {
            Self::Network(_) | Self::Parse(_) => Outcome::Retry,
            // 8 operation failed, 11 service offline, 16 temporarily unavailable, 29 rate limit exceeded.
            Self::Api {
                code: 8 | 11 | 16 | 29,
                ..
            } => Outcome::Retry,
            // 9 invalid session key.
            Self::Api { code: 9, .. } => Outcome::SessionLost,
            Self::Api { .. } => Outcome::Fatal,
        }
    }
}

/// The JSON of an answer, or why there is none. Last.fm reports errors as `{"error": 9, "message": "..."}`, with
/// an HTTP status that is not always 200.
fn parse_response(status: u16, body: &str) -> Result<Value, ApiError> {
    let value: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) if status >= 500 || status == 429 => {
            return Err(ApiError::Network(format!("HTTP {status}, {e}")));
        }
        Err(e) => return Err(ApiError::Parse(e.to_string())),
    };
    // Last.fm sends a number. A numeric string is read the same way, and any other `error` still counts as a failure
    // (code 0), so its message reaches the log instead of the answer being mistaken for a good one.
    if let Some(err) = value.get("error").filter(|e| !e.is_null()) {
        let code = err
            .as_u64()
            .or_else(|| err.as_str().and_then(|s| s.trim().parse().ok()))
            .unwrap_or(0);
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("no message")
            .to_owned();
        return Err(ApiError::Api {
            code: u32::try_from(code).unwrap_or(u32::MAX),
            message,
        });
    }
    if status >= 400 {
        return Err(ApiError::Network(format!("HTTP {status}")));
    }
    Ok(value)
}

/// An unauthorized request token. Last.fm documents 32 hexadecimal characters, but anything printable without
/// spaces is taken (it is percent-encoded where it goes into an address), so an unusual token does not stop the
/// sign-in. The error says what was wrong with it, never what it was.
fn parse_token(v: &Value) -> Result<String, ApiError> {
    let raw = match v.get("token") {
        None => {
            return Err(ApiError::Parse(format!(
                "no request token; the answer holds: {}",
                shape(v)
            )));
        }
        Some(Value::String(s)) => s.trim(),
        Some(_) => return Err(ApiError::Parse("the request token is not text".into())),
    };
    let chars = raw.chars().count();
    if raw.is_empty() {
        Err(ApiError::Parse("the request token is empty".into()))
    } else if raw.len() > 128 {
        Err(ApiError::Parse(format!(
            "the request token is too long ({chars} characters)"
        )))
    } else if !raw.bytes().all(|b| b.is_ascii_graphic()) {
        Err(ApiError::Parse(format!(
            "the request token has spaces or characters that are not plain ASCII ({chars} characters)"
        )))
    } else {
        Ok(raw.to_owned())
    }
}

/// `s` written for the query part of an address: everything but letters, digits and `-_.~` becomes `%XX`.
fn encode_component(s: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(b >> 4)]));
            out.push(char::from(HEX[usize::from(b & 0x0f)]));
        }
    }
    out
}

/// What an answer looked like, for the log: an object's key names, never its values (they can hold tokens and keys).
fn shape(v: &Value) -> String {
    match v {
        Value::Object(m) if m.is_empty() => "an empty object".into(),
        Value::Object(m) => m
            .keys()
            .take(8)
            .map(|k| k.chars().take(30).collect::<String>())
            .collect::<Vec<_>>()
            .join(", "),
        Value::Array(_) => "an array".into(),
        _ => "something that is not an object".into(),
    }
}

/// The session key and the user's name from `auth.getSession`.
fn parse_session(v: &Value) -> Result<(String, String), ApiError> {
    let text = |p: &str| {
        v.pointer(p)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    };
    match (text("/session/key"), text("/session/name")) {
        (Some(key), Some(name)) => Ok((key.to_owned(), name.to_owned())),
        _ => Err(ApiError::Parse("no session".into())),
    }
}

/// How many scrobbles Last.fm accepted and ignored, from a `track.scrobble` answer.
fn parse_scrobbles(v: &Value) -> Result<(u64, u64), ApiError> {
    let count = |name: &str| {
        let n = v.pointer(&format!("/scrobbles/@attr/{name}"))?;
        n.as_u64()
            .or_else(|| n.as_str().and_then(|s| s.trim().parse().ok()))
    };
    match (count("accepted"), count("ignored")) {
        (Some(accepted), Some(ignored)) => Ok((accepted, ignored)),
        _ => Err(ApiError::Parse("no scrobble counts".into())),
    }
}

/// The address where the user allows Pyxee to use their Last.fm account.
fn auth_page(app: &App, token: &str) -> String {
    format!(
        "{AUTH_URL}?api_key={}&token={}",
        app.key,
        encode_component(token)
    )
}

fn param(name: &str, value: impl Into<String>) -> (String, String) {
    (name.to_owned(), value.into())
}

struct Api {
    app: App,
    endpoint: String,
    agent: ureq::Agent,
}

impl Api {
    fn new(app: App) -> Self {
        Self {
            app,
            endpoint: API_URL.to_owned(),
            agent: ureq::AgentBuilder::new()
                .timeout_connect(CONNECT_TIMEOUT)
                .timeout(HTTP_TIMEOUT)
                .user_agent(concat!("Pyxee/", env!("CARGO_PKG_VERSION")))
                .build(),
        }
    }

    /// One signed call. Everything goes in the body of a POST, so no key or signature ends up in an address.
    fn call(
        &self,
        method: &str,
        mut params: Vec<(String, String)>,
        session: Option<&str>,
    ) -> Result<Value, ApiError> {
        params.push(param("method", method));
        params.push(param("api_key", self.app.key.as_str()));
        if let Some(sk) = session {
            params.push(param("sk", sk));
        }
        let signature = sign(&params, &self.app.secret);
        params.push(param("api_sig", signature));
        params.push(param("format", "json"));
        let form: Vec<(&str, &str)> = params
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        match self.agent.post(&self.endpoint).send_form(&form) {
            Ok(r) => {
                let status = r.status();
                let body = r
                    .into_string()
                    .map_err(|e| ApiError::Network(e.to_string()))?;
                parse_response(status, &body)
            }
            Err(ureq::Error::Status(status, r)) => {
                parse_response(status, &r.into_string().unwrap_or_default())
            }
            Err(ureq::Error::Transport(t)) => Err(ApiError::Network(t.to_string())),
        }
    }

    fn get_token(&self) -> Result<String, ApiError> {
        parse_token(&self.call("auth.getToken", Vec::new(), None)?)
    }

    fn get_session(&self, token: &str) -> Result<(String, String), ApiError> {
        parse_session(&self.call("auth.getSession", vec![param("token", token)], None)?)
    }

    fn now_playing(&self, session: &str, song: &Song) -> Result<(), ApiError> {
        let mut params = vec![param("artist", &*song.artist), param("track", &*song.title)];
        if let Some(ms) = song.duration_ms {
            params.push(param("duration", (ms / 1000).to_string()));
        }
        self.call("track.updateNowPlaying", params, Some(session))
            .map(drop)
    }

    /// Sends up to `BATCH_MAX` scrobbles; returns how many Last.fm accepted and how many it ignored.
    fn scrobble(&self, session: &str, batch: &[&Scrobble]) -> Result<(u64, u64), ApiError> {
        let mut params = Vec::with_capacity(batch.len() * 4);
        for (i, s) in batch.iter().enumerate() {
            params.push(param(&format!("artist[{i}]"), &*s.song.artist));
            params.push(param(&format!("track[{i}]"), &*s.song.title));
            params.push(param(&format!("timestamp[{i}]"), s.timestamp.to_string()));
            if let Some(ms) = s.song.duration_ms {
                params.push(param(&format!("duration[{i}]"), (ms / 1000).to_string()));
            }
        }
        parse_scrobbles(&self.call("track.scrobble", params, Some(session))?)
    }
}

/// Opens the address in the default browser.
fn open_url(url: &str) -> Result<(), String> {
    let program = if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    // Explorer reports failure even when it opened the page, so only a failure to start counts.
    std::process::Command::new(program)
        .arg(url)
        .spawn()
        .map(drop)
        .map_err(|e| format!("could not start {program}: {e}"))
}

// ---------------------------------------------------------------------------------------------------------------
// The worker thread
// ---------------------------------------------------------------------------------------------------------------

/// What the worker tells the rest of the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Something the listener should read.
    Notice(String),
    /// The user allowed Pyxee: keep this session key for the next start.
    Session(String),
    /// Last.fm no longer accepts the saved session key: forget it.
    SessionLost,
}

enum Msg {
    NowPlaying(Song),
    Scrobble(Scrobble),
    Shutdown(Sender<()>),
}

/// Waiting for the user to allow Pyxee in the browser.
struct PendingAuth {
    token: String,
    deadline: Instant,
    next_poll: Instant,
}

struct Worker<F: Fn(Event)> {
    api: Api,
    emit: F,
    session: Option<String>,
    auth: Option<PendingAuth>,
    pending: VecDeque<Scrobble>,
    retry_at: Option<Instant>,
    backoff: Duration,
    /// After a failure that trying again cannot fix everything is dropped until the next start.
    stopped: bool,
    /// Only the first failure of a streak is a warning; the rest would fill the log.
    warned: bool,
}

impl<F: Fn(Event)> Worker<F> {
    fn new(app: App, session: Option<String>, emit: F) -> Self {
        Self {
            api: Api::new(app),
            emit,
            session,
            auth: None,
            pending: VecDeque::new(),
            retry_at: None,
            backoff: BACKOFF_MIN,
            stopped: false,
            warned: false,
        }
    }

    fn notice(&self, message: impl Into<String>) {
        (self.emit)(Event::Notice(message.into()));
    }

    /// Stops for good (until the next start) and tells the listener why.
    fn give_up(&mut self, why: &str) {
        log::warn!("last.fm: stopped ({why})");
        self.notice(format!(
            "{why} Turn Last.fm scrobbling off and on in Settings to try again."
        ));
        self.stopped = true;
        self.auth = None;
        self.pending.clear();
    }

    /// Asks for a request token, again after a short wait when the failure may be temporary.
    fn fetch_token(&self) -> Result<String, ApiError> {
        let mut tries = 1;
        loop {
            match self.api.get_token() {
                Err(e) if e.outcome() == Outcome::Retry && tries < TOKEN_TRIES => {
                    log::debug!("last.fm: no request token yet ({e}); asking again");
                    tries += 1;
                    std::thread::sleep(TOKEN_RETRY_WAIT);
                }
                other => return other,
            }
        }
    }

    /// Asks Last.fm for a request token and opens the page where the user allows Pyxee.
    fn begin_auth(&mut self) {
        let token = match self.fetch_token() {
            Ok(t) => t,
            Err(e) => {
                log::warn!("last.fm: could not start signing in: {e}");
                self.give_up(match e {
                    ApiError::Network(_) => "Last.fm could not be reached to sign in.",
                    ApiError::Parse(_) => {
                        "Last.fm answered in a way Pyxee could not use to sign in."
                    }
                    ApiError::Api { .. } => "Last.fm refused the request to sign in.",
                });
                return;
            }
        };
        let url = auth_page(&self.api.app, &token);
        match open_url(&url) {
            Ok(()) => self.notice(
                "Last.fm: allow Pyxee in the page that opened in your browser. Scrobbling starts once you have.",
            ),
            Err(e) => {
                log::warn!("last.fm: could not open the browser: {e}");
                self.notice(format!("Last.fm: open this page to allow Pyxee: {url}"));
            }
        }
        let now = Instant::now();
        self.auth = Some(PendingAuth {
            token,
            deadline: now + AUTH_WINDOW,
            next_poll: now + AUTH_POLL,
        });
    }

    /// Asks whether the user has allowed Pyxee yet.
    fn poll_auth(&mut self) {
        let Some((token, deadline)) = self.auth.as_ref().map(|a| (a.token.clone(), a.deadline))
        else {
            return;
        };
        match self.api.get_session(&token) {
            Ok((key, name)) => {
                log::info!("last.fm: signed in");
                self.auth = None;
                self.session = Some(key.clone());
                (self.emit)(Event::Session(key));
                self.notice(format!("Last.fm is connected as {name}."));
            }
            // 4 invalid token, 15 token expired.
            Err(ApiError::Api { code: 4 | 15, .. }) => {
                self.give_up("The Last.fm sign-in link expired.");
            }
            Err(e) => {
                // 14 is "not allowed yet"; a network problem is probably temporary. Keep asking until the time is up.
                let waiting =
                    matches!(e, ApiError::Api { code: 14, .. }) || e.outcome() == Outcome::Retry;
                if !waiting {
                    log::warn!("last.fm: sign-in failed: {e}");
                    self.give_up("Last.fm refused the sign-in.");
                } else if Instant::now() >= deadline {
                    self.give_up("The Last.fm sign-in was not finished.");
                } else {
                    log::debug!("last.fm: not signed in yet: {e}");
                    if let Some(a) = self.auth.as_mut() {
                        a.next_poll = Instant::now() + AUTH_POLL;
                    }
                }
            }
        }
    }

    fn enqueue(&mut self, s: Scrobble) {
        if self.stopped {
            return;
        }
        if self.pending.len() >= PENDING_MAX {
            self.pending.pop_front();
        }
        self.pending.push_back(s);
    }

    fn now_playing(&mut self, song: &Song) {
        let (false, Some(session)) = (self.stopped, self.session.as_deref()) else {
            return;
        };
        match self.api.now_playing(session, song) {
            Ok(()) => log::debug!("last.fm: now playing sent"),
            // Stale a moment later, so it is not retried.
            Err(e) => match e.outcome() {
                Outcome::Retry => log::debug!("last.fm: now playing not sent: {e}"),
                other => self.failed(&e, other),
            },
        }
    }

    /// Sends what is waiting, unless a back-off is still running.
    fn flush(&mut self) {
        let (false, Some(session)) = (self.stopped, self.session.clone()) else {
            return;
        };
        if self.pending.is_empty() || self.retry_at.is_some_and(|t| Instant::now() < t) {
            return;
        }
        let count = self.pending.len().min(BATCH_MAX);
        let result = {
            let batch: Vec<&Scrobble> = self.pending.iter().take(count).collect();
            self.api.scrobble(&session, &batch)
        };
        match result {
            Ok((accepted, ignored)) => {
                self.pending.drain(..count);
                self.backoff = BACKOFF_MIN;
                self.warned = false;
                self.retry_at = None;
                log::info!("last.fm: scrobbled {accepted}, ignored {ignored}");
            }
            Err(e) => match e.outcome() {
                Outcome::Retry => {
                    if self.warned {
                        log::debug!(
                            "last.fm: scrobbles not sent ({e}); retrying in {:?}",
                            self.backoff
                        );
                    } else {
                        self.warned = true;
                        log::warn!("last.fm: scrobbles not sent ({e}); will retry");
                    }
                    self.retry_at = Some(Instant::now() + self.backoff);
                    self.backoff = (self.backoff * 2).min(BACKOFF_MAX);
                }
                other => self.failed(&e, other),
            },
        }
    }

    /// A failure that waiting does not fix.
    fn failed(&mut self, e: &ApiError, outcome: Outcome) {
        if outcome == Outcome::SessionLost {
            log::warn!("last.fm: the saved sign-in is no longer valid ({e})");
            self.session = None;
            (self.emit)(Event::SessionLost);
            self.give_up("Last.fm no longer accepts the saved sign-in.");
        } else {
            log::warn!("last.fm: {e}");
            let why = match e {
                ApiError::Api { message, .. } => format!("Last.fm refused a request ({message})."),
                _ => "Last.fm refused a request.".to_string(),
            };
            self.give_up(&why);
        }
    }

    /// How long until something is due: the next sign-in check, or the next try at sending what is waiting.
    fn due_in(&self, now: Instant) -> Option<Duration> {
        let mut at = self.auth.as_ref().map(|a| a.next_poll);
        if !self.stopped && self.session.is_some() && !self.pending.is_empty() {
            let t = self.retry_at.unwrap_or(now);
            at = Some(at.map_or(t, |a| a.min(t)));
        }
        at.map(|t| t.saturating_duration_since(now))
    }

    fn tick(&mut self) {
        if self
            .auth
            .as_ref()
            .is_some_and(|a| Instant::now() >= a.next_poll)
        {
            self.poll_auth();
        }
        self.flush();
    }

    /// One last try at sending what is waiting, before the app quits.
    fn finish(&mut self) {
        self.retry_at = None;
        self.flush();
    }
}

fn run(app: App, session: Option<String>, emit: impl Fn(Event), rx: &Receiver<Msg>) {
    let mut worker = Worker::new(app, session, emit);
    if worker.session.is_none() {
        worker.begin_auth();
    }
    loop {
        let msg = match worker.due_in(Instant::now()) {
            Some(wait) => match rx.recv_timeout(wait) {
                Ok(m) => Some(m),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break,
            },
            None => match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => break,
            },
        };
        match msg {
            Some(Msg::NowPlaying(song)) => worker.now_playing(&song),
            Some(Msg::Scrobble(s)) => worker.enqueue(s),
            Some(Msg::Shutdown(done)) => {
                worker.finish();
                let _ = done.send(());
                return;
            }
            None => {}
        }
        worker.tick();
    }
    worker.finish();
}

// ---------------------------------------------------------------------------------------------------------------
// What the rest of the app sees
// ---------------------------------------------------------------------------------------------------------------

pub struct Lastfm {
    tx: Sender<Msg>,
    tracker: Mutex<Tracker>,
}

/// Starts the scrobbling thread. `session` is the key saved by an earlier sign-in; without one the browser opens
/// for the user to allow Pyxee. The error says, in words for the listener, why scrobbling is not available.
/// Never fails the app.
pub fn init(
    session: Option<String>,
    emit: impl Fn(Event) + Send + 'static,
) -> Result<Lastfm, String> {
    let app = resolve_app(
        option_env!("PYXEE_LASTFM_API_KEY"),
        option_env!("PYXEE_LASTFM_API_SECRET"),
    )
    .map_err(|e| {
        log::warn!("last.fm: off ({e})");
        "Last.fm scrobbling is not available in this build of Pyxee.".to_string()
    })?
    .ok_or_else(|| {
        log::info!("last.fm: off (this build has no Last.fm application)");
        "Last.fm scrobbling is not available in this build of Pyxee.".to_string()
    })?;
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("pyxee-lastfm".into())
        .spawn(move || run(app, session, emit, &rx))
        .map_err(|e| {
            log::warn!("last.fm: off (could not start its thread: {e})");
            "Last.fm scrobbling could not be started.".to_string()
        })?;
    log::info!("last.fm: on");
    Ok(Lastfm {
        tx,
        tracker: Mutex::new(Tracker::default()),
    })
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

impl Lastfm {
    /// Called with every player state. Cheap and non-blocking: most states send nothing.
    pub fn on_state(
        &self,
        phase: Phase,
        id: Option<u64>,
        track: Option<&TrackDto>,
        position_ms: u64,
    ) {
        let actions = match self.tracker.lock() {
            Ok(mut t) => t.observe(phase, id, track, position_ms, now_secs()),
            Err(_) => return,
        };
        for action in actions {
            let _ = self.tx.send(match action {
                Action::NowPlaying(song) => Msg::NowPlaying(song),
                Action::Scrobble(s) => Msg::Scrobble(s),
            });
        }
    }

    /// Sends what is still waiting before the app exits, waiting only briefly.
    pub fn shutdown(&self) {
        let (done_tx, done_rx) = mpsc::channel();
        if self.tx.send(Msg::Shutdown(done_tx)).is_ok() {
            let _ = done_rx.recv_timeout(SHUTDOWN_WAIT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_error_code_is_read_as_a_number_or_a_numeric_string() {
        for body in [
            r#"{"error":10,"message":"bad key"}"#,
            r#"{"error":"10","message":"bad key"}"#,
        ] {
            assert_eq!(
                parse_response(200, body),
                Err(ApiError::Api {
                    code: 10,
                    message: "bad key".into()
                }),
                "{body}"
            );
        }
    }

    #[test]
    fn an_error_of_another_kind_is_still_a_failure_with_its_message() {
        let got = parse_response(200, r#"{"error":"nope","message":"why"}"#);
        assert_eq!(
            got,
            Err(ApiError::Api {
                code: 0,
                message: "why".into()
            })
        );
        assert!(parse_response(200, r#"{"error":null,"token":"abc"}"#).is_ok());
    }

    #[test]
    fn a_missing_token_names_the_keys_of_the_answer_and_not_their_values() {
        let v: Value = serde_json::from_str(r#"{"status":"ok","data":"secret123"}"#).unwrap();
        let msg = parse_token(&v).unwrap_err().to_string();
        assert!(msg.contains("status") && msg.contains("data"), "{msg}");
        assert!(!msg.contains("secret123"), "{msg}");
        let ok: Value =
            serde_json::from_str(r#"{"token":"cf45fe5a3e3cebe168480a086d7fe481"}"#).unwrap();
        assert_eq!(
            parse_token(&ok).unwrap(),
            "cf45fe5a3e3cebe168480a086d7fe481"
        );
    }

    #[test]
    fn an_unusual_token_is_taken_and_a_bad_one_is_described_without_its_value() {
        let token = |t: &str| parse_token(&serde_json::json!({ "token": t }));
        assert_eq!(token("ab-_.12").unwrap(), "ab-_.12");
        assert_eq!(token(" abc ").unwrap(), "abc");
        let long = "a".repeat(129);
        for bad in ["", "a b", "caf\u{e9}", long.as_str()] {
            let msg = token(bad).unwrap_err().to_string();
            assert!(msg.contains("request token"), "{msg}");
            assert!(!bad.is_empty() || msg.contains("empty"), "{msg}");
            assert!(bad.len() < 3 || !msg.contains(bad), "{msg}");
        }
        let msg = parse_token(&serde_json::json!({ "token": 5 }))
            .unwrap_err()
            .to_string();
        assert!(msg.contains("not text"), "{msg}");
    }

    #[test]
    fn the_token_is_encoded_in_the_address_the_browser_opens() {
        let app = App {
            key: "0123456789abcdef0123456789abcdef".into(),
            secret: "fedcba9876543210fedcba9876543210".into(),
        };
        assert_eq!(
            auth_page(&app, "cf45fe5a"),
            "https://www.last.fm/api/auth/?api_key=0123456789abcdef0123456789abcdef&token=cf45fe5a"
        );
        assert!(auth_page(&app, "a&b=c d").ends_with("token=a%26b%3Dc%20d"));
    }
}
