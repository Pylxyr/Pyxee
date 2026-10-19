//! Discord Rich Presence: "Listening to Pyxee" with the current song, through Discord's local IPC (a named pipe on
//! Windows), using the `discord-rich-presence` crate. No bot token, no OAuth, no network.
//!
//! Presence is derived from the player state `shell::on_state` already receives; this module adds no playback logic.
//! Everything that decides something is a pure function or struct (`build_spec`, `Publisher`, `RateLimit`,
//! `parse_client_id`) and is unit tested; the only part that needs a real Discord client is `Worker`.
//!
//! `on_state` runs on whatever thread publishes player state, so it never touches the pipe. It only hands a
//! small message to the `pyxee-discord` thread, which owns the connection, reconnects with a bounded back-off while
//! something is playing, and keeps to Discord's update rate limit. If Discord is not running nothing else changes.

use crate::dto::TrackDto;
use discord_rich_presence::activity::{Activity, ActivityType, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use pyxee_shell::progress::Phase;
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The Discord application ID (Developer Portal -> Application -> "Application ID"). There is no built-in default.
const ENV_CLIENT_ID: &str = "PYXEE_DISCORD_CLIENT_ID";
/// Discord shows at most 128 characters per line and rejects lines shorter than 2.
const MAX_TEXT: usize = 128;
/// How far the playing clock may wander from what Discord shows before the bar is corrected: a seek, a stall, or
/// slow drift. Normal one-second position ticks stay far below this and send nothing.
const DRIFT_MS: i64 = 2_000;
/// Discord allows about 5 updates per 20 seconds; stay under it.
const RATE_WINDOW: Duration = Duration::from_secs(20);
const RATE_MAX_IN_WINDOW: usize = 4;
/// Never two updates closer than this, so a quick burst of state changes collapses into the last one.
const MIN_GAP: Duration = Duration::from_millis(500);
const BACKOFF_MIN: Duration = Duration::from_secs(2);
const BACKOFF_MAX: Duration = Duration::from_secs(30);
/// How long quitting waits for the presence to be cleared before giving up on it.
const SHUTDOWN_WAIT: Duration = Duration::from_millis(1_000);

// ---------------------------------------------------------------------------------------------------------------
// Pure decisions
// ---------------------------------------------------------------------------------------------------------------

/// A Discord application ID is a snowflake: 17 to 20 digits. Anything else disables the integration.
pub fn parse_client_id(raw: &str) -> Option<String> {
    let id = raw.trim();
    let ok = (17..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit());
    ok.then(|| id.to_owned())
}

/// The runtime environment wins over an ID baked in at build time (`PYXEE_DISCORD_CLIENT_ID=... cargo build`).
fn resolve_client_id(
    runtime: Option<&str>,
    build_time: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(raw) = runtime
        .filter(|r| !r.trim().is_empty())
        .or(build_time.filter(|b| !b.trim().is_empty()))
    else {
        return Ok(None);
    };
    parse_client_id(raw)
        .map(Some)
        .ok_or_else(|| format!("{ENV_CLIENT_ID} is not a Discord application ID (17 to 20 digits)"))
}

/// What Discord should show for the song.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Audible: a progress bar anchored to the wall clock (unix milliseconds).
    Playing { start_ms: i64, end_ms: Option<i64> },
    /// Between songs or re-buffering: the song is shown, but with no bar until it is audible.
    Loading,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub title: String,
    pub artist: Option<String>,
    /// Cover art shown as the large image: an `https` URL Discord can fetch (see `cover_url`).
    pub cover: Option<String>,
    pub mode: Mode,
}

/// Discord loads the large image itself, so it must be a short `https` URL. Local files, `data:` URIs and anything
/// else are left out (Discord then shows the application's own icon).
fn cover_url(thumbnail: Option<&str>) -> Option<String> {
    let t = thumbnail?.trim();
    let ok = t.len() <= 256
        && t.len() > "https://".len()
        && t.starts_with("https://")
        && !t.chars().any(char::is_whitespace);
    ok.then(|| t.to_owned())
}

/// Fits a line to Discord's limits: trimmed, at most `MAX_TEXT` characters, at least 2.
fn clamp_text(s: &str) -> String {
    let s = s.trim();
    let mut out: String = if s.chars().count() > MAX_TEXT {
        let mut cut: String = s.chars().take(MAX_TEXT - 1).collect();
        cut.push('\u{2026}');
        cut
    } else {
        s.to_owned()
    };
    if out.chars().count() < 2 {
        out.push('\u{00A0}');
    }
    out
}

/// The presence for the current player state, or `None` when nothing should be shown: idle, failed, no song, or
/// paused (like Spotify, pausing takes the activity down; resuming puts it back with a fresh bar).
pub fn build_spec(
    phase: Phase,
    track: Option<&TrackDto>,
    position_ms: u64,
    now_ms: i64,
) -> Option<Spec> {
    let t = track?;
    let mode = match phase {
        Phase::Playing => {
            let start_ms = now_ms.saturating_sub(i64::try_from(position_ms).unwrap_or(i64::MAX));
            let end_ms = t
                .duration_ms
                .filter(|d| *d > 0)
                .map(|d| start_ms.saturating_add(i64::try_from(d).unwrap_or(i64::MAX)));
            Mode::Playing { start_ms, end_ms }
        }
        Phase::Loading => Mode::Loading,
        Phase::Idle | Phase::Paused | Phase::Failed => return None,
    };
    let title = if t.title.trim().is_empty() {
        "Unknown title"
    } else {
        &t.title
    };
    let artist = Some(t.artist.trim())
        .filter(|a| !a.is_empty())
        .map(clamp_text);
    Some(Spec {
        title: clamp_text(title),
        artist,
        cover: cover_url(t.thumbnail.as_deref()),
        mode,
    })
}

/// Whether Discord already shows (close enough to) `new`, so sending it again would only waste the rate limit.
fn same_enough(old: &Spec, new: &Spec) -> bool {
    if old.title != new.title || old.artist != new.artist || old.cover != new.cover {
        return false;
    }
    match (&old.mode, &new.mode) {
        (Mode::Loading, Mode::Loading) => true,
        (
            Mode::Playing {
                start_ms: a,
                end_ms: ea,
            },
            Mode::Playing {
                start_ms: b,
                end_ms: eb,
            },
        ) => (a - b).abs() <= DRIFT_MS && ea.is_some() == eb.is_some(),
        _ => false,
    }
}

/// What the worker thread is told.
enum Msg {
    Set(Spec),
    Clear,
    Shutdown(Sender<()>),
}

/// Remembers what Discord was last told and turns each new desired state into "send this" or "nothing to do".
#[derive(Default)]
struct Publisher {
    shown: Option<Spec>,
}

impl Publisher {
    fn decide(&mut self, next: Option<Spec>) -> Option<Msg> {
        match (&self.shown, next) {
            (None, None) => None,
            (Some(_), None) => {
                self.shown = None;
                Some(Msg::Clear)
            }
            (None, Some(s)) => {
                self.shown = Some(s.clone());
                Some(Msg::Set(s))
            }
            (Some(old), Some(s)) => {
                if same_enough(old, &s) {
                    None
                } else {
                    self.shown = Some(s.clone());
                    Some(Msg::Set(s))
                }
            }
        }
    }
}

/// Sliding-window limiter for updates sent to Discord.
#[derive(Default)]
struct RateLimit {
    sent: VecDeque<Instant>,
}

impl RateLimit {
    /// When the next update may go out.
    fn ready_at(&mut self, now: Instant) -> Instant {
        while self
            .sent
            .front()
            .is_some_and(|t| now.saturating_duration_since(*t) >= RATE_WINDOW)
        {
            self.sent.pop_front();
        }
        let after_gap = self.sent.back().map_or(now, |t| *t + MIN_GAP);
        let after_window = if self.sent.len() >= RATE_MAX_IN_WINDOW {
            self.sent.front().map_or(now, |t| *t + RATE_WINDOW)
        } else {
            now
        };
        after_gap.max(after_window).max(now)
    }

    fn record(&mut self, now: Instant) {
        self.sent.push_back(now);
    }
}

fn to_activity(spec: &Spec) -> Activity<'_> {
    // Discord's own layout does the rest: bold title, "by Artist", the cover, and the bar.
    let mut activity = Activity::new()
        .activity_type(ActivityType::Listening)
        .details(spec.title.as_str());
    if let Some(artist) = &spec.artist {
        activity = activity.state(artist.as_str());
    }
    if let Some(cover) = &spec.cover {
        activity = activity.assets(Assets::new().large_image(cover.as_str()));
    }
    if let Mode::Playing { start_ms, end_ms } = spec.mode {
        let mut ts = Timestamps::new().start(start_ms);
        if let Some(end) = end_ms {
            ts = ts.end(end);
        }
        activity = activity.timestamps(ts);
    }
    activity
}

// ---------------------------------------------------------------------------------------------------------------
// The connection (needs a real Discord to be exercised)
// ---------------------------------------------------------------------------------------------------------------

struct Worker {
    client_id: String,
    client: Option<DiscordIpcClient>,
    next_connect: Instant,
    backoff: Duration,
    limit: RateLimit,
    /// Only the first failure of a streak is a warning; the rest would fill the log.
    warned: bool,
}

impl Worker {
    fn new(client_id: String) -> Self {
        Self {
            client_id,
            client: None,
            next_connect: Instant::now(),
            backoff: BACKOFF_MIN,
            limit: RateLimit::default(),
            warned: false,
        }
    }

    /// How long until the pending request can be attempted: the rate limit, and the back-off while not connected.
    fn ready_in(&mut self, now: Instant) -> Duration {
        let mut at = self.limit.ready_at(now);
        if self.client.is_none() {
            at = at.max(self.next_connect);
        }
        at.saturating_duration_since(now)
    }

    fn connect(&mut self, now: Instant) -> bool {
        let mut client = DiscordIpcClient::new(&self.client_id);
        match client.connect() {
            Ok(()) => {
                log::info!("discord presence: connected to the Discord client");
                self.client = Some(client);
                self.backoff = BACKOFF_MIN;
                self.warned = false;
                true
            }
            Err(e) => {
                if self.warned {
                    log::debug!(
                        "discord presence: Discord is not reachable ({e}); retrying in {:?}",
                        self.backoff
                    );
                } else {
                    self.warned = true;
                    log::warn!(
                        "discord presence: Discord is not reachable ({e}); will retry while music plays"
                    );
                }
                self.next_connect = now + self.backoff;
                self.backoff = (self.backoff * 2).min(BACKOFF_MAX);
                false
            }
        }
    }

    fn drop_client(&mut self, now: Instant) {
        if let Some(mut c) = self.client.take() {
            let _ = c.close();
        }
        self.next_connect = now + BACKOFF_MIN;
    }

    /// Tries to make Discord show `want` (`None` = nothing). `true` means done; `false` means try again later.
    fn apply(&mut self, want: &Option<Spec>) -> bool {
        let now = Instant::now();
        if self.client.is_none() {
            if want.is_none() {
                return true; // nothing to clear, and no reason to connect
            }
            if !self.connect(now) {
                return false;
            }
        }
        let Some(client) = self.client.as_mut() else {
            return false;
        };
        let result = match want {
            Some(spec) => client.set_activity(to_activity(spec)),
            None => client.clear_activity(),
        };
        match result {
            Ok(()) => {
                self.limit.record(Instant::now());
                true
            }
            Err(e) => {
                log::warn!("discord presence: update failed ({e}); reconnecting");
                self.drop_client(now);
                false
            }
        }
    }

    fn shutdown(&mut self) {
        if let Some(mut c) = self.client.take() {
            if let Err(e) = c.clear_activity() {
                log::debug!("discord presence: could not clear on exit: {e}");
            }
            let _ = c.close();
        }
    }
}

fn run(client_id: String, rx: &Receiver<Msg>) {
    let mut worker = Worker::new(client_id);
    // The newest thing Discord should show; `Some(None)` is "show nothing". Older requests are simply replaced.
    let mut want: Option<Option<Spec>> = None;
    loop {
        let msg = if want.is_some() {
            let wait = worker.ready_in(Instant::now());
            match rx.recv_timeout(wait) {
                Ok(m) => Some(m),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => break,
            }
        };
        match msg {
            Some(Msg::Set(s)) => {
                want = Some(Some(s));
                continue;
            }
            Some(Msg::Clear) => {
                want = Some(None);
                continue;
            }
            Some(Msg::Shutdown(done)) => {
                worker.shutdown();
                let _ = done.send(());
                return;
            }
            None => {}
        }
        if let Some(w) = want.take() {
            if !worker.apply(&w) {
                want = Some(w);
            }
        }
    }
    worker.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// What the rest of the app sees
// ---------------------------------------------------------------------------------------------------------------

pub struct Discord {
    tx: Sender<Msg>,
    publisher: Mutex<Publisher>,
}

/// Starts the presence thread, or returns `None` (and says why in the log) when no valid application ID is
/// configured or the thread cannot start. Never fails the app.
pub fn init() -> Option<Discord> {
    let runtime = std::env::var(ENV_CLIENT_ID).ok();
    let client_id =
        match resolve_client_id(runtime.as_deref(), option_env!("PYXEE_DISCORD_CLIENT_ID")) {
            Ok(Some(id)) => id,
            Ok(None) => {
                log::info!("discord presence: off ({ENV_CLIENT_ID} is not set)");
                return None;
            }
            Err(e) => {
                log::warn!("discord presence: off ({e})");
                return None;
            }
        };
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("pyxee-discord".into())
        .spawn(move || run(client_id, &rx));
    if let Err(e) = spawned {
        log::warn!("discord presence: off (could not start its thread: {e})");
        return None;
    }
    log::info!("discord presence: on");
    Some(Discord {
        tx,
        publisher: Mutex::new(Publisher::default()),
    })
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

impl Discord {
    /// Called with every player state. Cheap and non-blocking: unchanged states send nothing.
    pub fn on_state(&self, phase: Phase, track: Option<&TrackDto>, position_ms: u64) {
        let spec = build_spec(phase, track, position_ms, now_ms());
        let Ok(mut publisher) = self.publisher.lock() else {
            return;
        };
        if let Some(msg) = publisher.decide(spec) {
            let _ = self.tx.send(msg);
        }
    }

    /// Clears the presence and closes the connection before the app exits, waiting only briefly.
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

    fn track(title: &str, artist: &str, duration_ms: Option<u64>) -> TrackDto {
        TrackDto {
            key: "k".into(),
            title: title.into(),
            artist: artist.into(),
            duration_ms,
            thumbnail: None,
        }
    }

    const NOW: i64 = 1_700_000_000_000;

    fn playing(t: &TrackDto, pos: u64, now: i64) -> Spec {
        build_spec(Phase::Playing, Some(t), pos, now).unwrap()
    }

    #[test]
    fn client_ids_must_look_like_snowflakes() {
        assert_eq!(
            parse_client_id(" 123456789012345678 "),
            Some("123456789012345678".to_owned())
        );
        assert_eq!(parse_client_id("1234567890123456"), None); // 16 digits
        assert_eq!(parse_client_id("123456789012345678901"), None); // 21 digits
        assert_eq!(parse_client_id("12345678901234567x"), None);
        assert_eq!(parse_client_id(""), None);
    }

    #[test]
    fn the_runtime_id_beats_the_build_time_id_and_nothing_means_off() {
        let a = "111111111111111111";
        let b = "222222222222222222";
        assert_eq!(resolve_client_id(Some(a), Some(b)), Ok(Some(a.to_owned())));
        assert_eq!(resolve_client_id(None, Some(b)), Ok(Some(b.to_owned())));
        assert_eq!(
            resolve_client_id(Some("  "), Some(b)),
            Ok(Some(b.to_owned()))
        );
        assert_eq!(resolve_client_id(None, None), Ok(None));
        assert!(resolve_client_id(Some("not-an-id"), Some(b)).is_err());
    }

    #[test]
    fn idle_failed_and_no_song_show_nothing() {
        let t = track("Song", "Artist", Some(1000));
        assert_eq!(build_spec(Phase::Idle, Some(&t), 0, NOW), None);
        assert_eq!(build_spec(Phase::Failed, Some(&t), 0, NOW), None);
        assert_eq!(build_spec(Phase::Playing, None, 0, NOW), None);
    }

    #[test]
    fn playing_anchors_the_bar_to_the_position() {
        let t = track("Song", "Artist", Some(200_000));
        let s = playing(&t, 30_000, NOW);
        assert_eq!(
            s.mode,
            Mode::Playing {
                start_ms: NOW - 30_000,
                end_ms: Some(NOW - 30_000 + 200_000)
            }
        );
        assert_eq!(s.title, "Song");
        assert_eq!(s.artist.as_deref(), Some("Artist"));
    }

    #[test]
    fn an_unknown_or_zero_duration_has_no_end() {
        for d in [None, Some(0)] {
            let s = playing(&track("Song", "A", d), 5_000, NOW);
            assert_eq!(
                s.mode,
                Mode::Playing {
                    start_ms: NOW - 5_000,
                    end_ms: None
                }
            );
        }
    }

    #[test]
    fn paused_shows_nothing_and_loading_shows_the_song_without_a_bar() {
        let t = track("Song", "Artist", Some(1000));
        assert_eq!(build_spec(Phase::Paused, Some(&t), 500, NOW), None);
        let loading = build_spec(Phase::Loading, Some(&t), 0, NOW).unwrap();
        assert_eq!(loading.mode, Mode::Loading);
        let json = serde_json::to_value(to_activity(&loading)).unwrap();
        assert_eq!(json["details"], "Song");
        assert_eq!(json["state"], "Artist");
        assert!(json.get("timestamps").is_none());
    }

    #[test]
    fn only_short_https_thumbnails_become_cover_art() {
        let ok = "https://i.ytimg.com/vi/abc/hqdefault.jpg";
        assert_eq!(cover_url(Some(ok)), Some(ok.to_owned()));
        assert_eq!(cover_url(Some("http://example.com/a.jpg")), None);
        assert_eq!(cover_url(Some(r"C:\Music\cover.jpg")), None);
        assert_eq!(cover_url(Some("data:image/png;base64,AAAA")), None);
        assert_eq!(cover_url(Some("https://")), None);
        assert_eq!(cover_url(Some("https://a b.com/x.jpg")), None);
        assert_eq!(
            cover_url(Some(&format!("https://x.com/{}", "a".repeat(260)))),
            None
        );
        assert_eq!(cover_url(None), None);
    }

    #[test]
    fn the_cover_goes_out_as_the_large_image() {
        let mut t = track("Song", "Artist", Some(10_000));
        t.thumbnail = Some("https://i.ytimg.com/vi/abc/hqdefault.jpg".into());
        let json = serde_json::to_value(to_activity(&playing(&t, 0, NOW))).unwrap();
        assert_eq!(
            json["assets"]["large_image"],
            "https://i.ytimg.com/vi/abc/hqdefault.jpg"
        );
        let json =
            serde_json::to_value(to_activity(&playing(&track("S", "A", None), 0, NOW))).unwrap();
        assert!(json.get("assets").is_none());
    }

    #[test]
    fn playing_activity_is_a_listening_bar() {
        let s = playing(&track("Song", "Artist", Some(10_000)), 1_000, NOW);
        let json = serde_json::to_value(to_activity(&s)).unwrap();
        assert_eq!(json["type"], 2);
        assert_eq!(json["details"], "Song");
        assert_eq!(json["state"], "Artist");
        assert_eq!(json["timestamps"]["start"], NOW - 1_000);
        assert_eq!(json["timestamps"]["end"], NOW - 1_000 + 10_000);
    }

    #[test]
    fn missing_metadata_is_handled() {
        let s = playing(&track("  ", "", None), 0, NOW);
        assert_eq!(s.title, "Unknown title");
        assert_eq!(s.artist, None);
        let json = serde_json::to_value(to_activity(&s)).unwrap();
        assert!(json.get("state").is_none());
    }

    #[test]
    fn lines_respect_discords_length_limits() {
        let long = "x".repeat(300);
        let s = playing(&track(&long, &long, None), 0, NOW);
        assert_eq!(s.title.chars().count(), MAX_TEXT);
        assert!(s.title.ends_with('\u{2026}'));
        assert_eq!(s.artist.unwrap().chars().count(), MAX_TEXT);
        // One character is padded to the 2 Discord requires; multi-byte characters count as one.
        assert_eq!(clamp_text("a").chars().count(), 2);
        assert_eq!(clamp_text(&"é".repeat(MAX_TEXT)).chars().count(), MAX_TEXT);
    }

    #[test]
    fn position_ticks_send_nothing_but_seeks_and_pauses_do() {
        let t = track("Song", "Artist", Some(200_000));
        let mut p = Publisher::default();
        assert!(matches!(
            p.decide(Some(playing(&t, 0, NOW))),
            Some(Msg::Set(_))
        ));
        // A second later the position is a second further: same bar.
        assert!(p.decide(Some(playing(&t, 1_000, NOW + 1_000))).is_none());
        assert!(p.decide(Some(playing(&t, 2_100, NOW + 2_100))).is_none());
        // A seek forward by a minute moves the bar.
        assert!(matches!(
            p.decide(Some(playing(&t, 63_000, NOW + 3_000))),
            Some(Msg::Set(_))
        ));
        // Pausing takes the activity down once; resuming puts it back with a fresh bar.
        let paused = build_spec(Phase::Paused, Some(&t), 63_000, NOW + 4_000);
        assert!(matches!(p.decide(paused.clone()), Some(Msg::Clear)));
        assert!(p.decide(paused).is_none());
        assert!(matches!(
            p.decide(Some(playing(&t, 63_000, NOW + 9_000))),
            Some(Msg::Set(_))
        ));
    }

    #[test]
    fn a_new_song_is_sent_and_idle_clears_once() {
        let a = track("Song A", "Artist", Some(1000));
        let b = track("Song B", "Artist", Some(1000));
        let mut p = Publisher::default();
        assert!(matches!(
            p.decide(Some(playing(&a, 0, NOW))),
            Some(Msg::Set(_))
        ));
        match p.decide(Some(playing(&b, 0, NOW))) {
            Some(Msg::Set(s)) => assert_eq!(s.title, "Song B"),
            _ => panic!("the new song should be sent"),
        }
        assert!(matches!(p.decide(None), Some(Msg::Clear)));
        assert!(p.decide(None).is_none());
    }

    #[test]
    fn a_restored_paused_song_shows_nothing_and_loading_then_playing_updates() {
        let t = track("Song", "Artist", Some(1000));
        let mut p = Publisher::default();
        assert!(p
            .decide(build_spec(Phase::Paused, Some(&t), 500, NOW))
            .is_none());
        // The user asked for a song: show it at once, then add the bar when it is audible.
        assert!(matches!(
            p.decide(build_spec(Phase::Loading, Some(&t), 0, NOW)),
            Some(Msg::Set(_))
        ));
        assert!(matches!(
            p.decide(Some(playing(&t, 0, NOW + 1_500))),
            Some(Msg::Set(_))
        ));
    }

    #[test]
    fn the_rate_limit_spaces_updates_and_caps_a_burst() {
        let t0 = Instant::now();
        let mut r = RateLimit::default();
        assert_eq!(r.ready_at(t0), t0);
        r.record(t0);
        assert_eq!(r.ready_at(t0), t0 + MIN_GAP);
        // Fill the window: the fifth update must wait for the first to leave it.
        let mut t = t0;
        for _ in 1..RATE_MAX_IN_WINDOW {
            t += MIN_GAP;
            r.record(t);
        }
        assert_eq!(r.ready_at(t), t0 + RATE_WINDOW);
        // Once the window has passed everything is free again.
        let later = t0 + RATE_WINDOW + Duration::from_secs(1);
        assert_eq!(r.ready_at(later), later);
    }
}
