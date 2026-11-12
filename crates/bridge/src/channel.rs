//! The core behind the C interface: the player, the audio engine, the library, the sleep timer and the media keys,
//! with every change pushed to the listener as a JSON event.
//!
//! A process-wide `App` ties the engine, the library service, the session file and the recorder (all in this crate,
//! see `lib.rs`) to the sleep timer, the media keys and the event listener.

use crate::advanced::{self, EnvState};
use crate::discord::{self, Discord};
use crate::dto::{
    state_dto, CommandDto, ExploreShelfDto, FilterInfoDto, GenreDto, NoticeDto, PlayerStateDto, PositionDto,
    SearchErrorDto, SearchFilterDto, SearchItemDto, SearchResponseDto, TrackDto,
};
use crate::engine::{Engine, Host};
use crate::library_service::LibraryService;
use crate::opener;
use crate::recorder::HistoryRecorder;
use crate::session;
use crate::shell_dto::{LyricsDto, SleepSettingDto, SleepStateDto, SleepStatusDto};
use crate::updater;
use pyxee_audio::output::{DeviceIdent, DeviceInfo, Output};
use pyxee_audio::pipeline::Volume;
use pyxee_audio::AudioError;
use pyxee_core::{Command, Effect, EngineEvent, Failure, Player};
use pyxee_lyrics::{fetch, Cooldown, Lyrics, Query, LRCLIB};
use pyxee_shell::audiodev::{Action as DeviceAction, DeviceRef, DeviceWatch, Observation, UserEffect};
use pyxee_shell::progress::Phase;
use pyxee_shell::sleep::{SleepSetting, SleepTimer};
use pyxee_sources::jsruntime::JsRuntime;
use pyxee_sources::local::LocalSource;
use pyxee_sources::ytdlp::{Cookies, YtDlpSource};
use pyxee_sources::ytmusic;
use pyxee_sources::{CompositeSource, MusicSource};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection};
use std::collections::{HashMap, VecDeque};
use std::ffi::{c_char, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub type EventCallback = extern "C" fn(*mut c_char);

const SAVE_EVERY: Duration = Duration::from_secs(5);
const DEVICE_CHECK_EVERY: Duration = Duration::from_millis(500);
const FALLBACK_RATE: u32 = 48_000;
const LYRICS_CACHE_MAX: usize = 64;
const PREFS_MAX_BYTES: usize = 64 * 1024;
const IDENTIFIER: &str = "dev.pyxee.app";

type TrackKey = Option<(u64, Option<String>)>;

#[derive(Default)]
struct Sent {
    track: TrackKey,
    playback: Option<(u8, u64)>,
}

struct App {
    player: Mutex<Player>,
    /// Order of publication; the UI keeps the state with the highest one.
    seq: AtomicU64,
    /// The last state published with `seq` and the position zeroed, to tell when only the position moved.
    last_published: Mutex<Option<PlayerStateDto>>,
    engine: Mutex<Sender<Effect>>,
    pending_rx: Mutex<Option<Receiver<Effect>>>,
    volume: Volume,
    library: Mutex<LibraryService>,
    source: Arc<dyn MusicSource>,
    session_path: Option<PathBuf>,
    last_saved: Mutex<Vec<u8>>,
    save_signal: Arc<session::SaveSignal>,
    on_event: Mutex<Option<EventCallback>>,
    sleep: Mutex<SleepTimer>,
    /// After a sleep timer fired, its fade-out stays at silence until playback starts again.
    restore_fade: AtomicBool,
    audible: Mutex<Option<u64>>,
    lyrics: Mutex<HashMap<String, Lyrics>>,
    cooldown: Mutex<Cooldown>,
    clock: Instant,
    media: Mutex<Option<MediaControls>>,
    sent: Mutex<Sent>,
    prefs_path: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    /// The `.env` file Settings > Advanced edits.
    env: EnvState,
    /// Discord Rich Presence, while it is switched on (and this build has a Discord application).
    discord: Mutex<Option<Discord>>,
}

static APP: OnceLock<App> = OnceLock::new();
static SPAWNED: Once = Once::new();
static CLOCK: OnceLock<thread::Thread> = OnceLock::new();

fn app() -> Option<&'static App> {
    APP.get()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64)
}

// ------------------------------------------------------------------ logging

struct StderrLog(log::LevelFilter);

impl log::Log for StderrLog {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.0
    }
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            eprintln!("[{}] {}: {}", record.level(), record.target(), record.args());
        }
    }
    fn flush(&self) {}
}

/// `PYXEE_LOG=error|warn|info|debug|trace|off` (default: warnings) writes the core's log to the process's stderr.
fn init_logging() {
    static LOGGER: OnceLock<StderrLog> = OnceLock::new();
    let level = match std::env::var("PYXEE_LOG").ok().as_deref() {
        Some("trace") => log::LevelFilter::Trace,
        Some("debug") => log::LevelFilter::Debug,
        Some("info") => log::LevelFilter::Info,
        Some("error") => log::LevelFilter::Error,
        Some("off") => log::LevelFilter::Off,
        _ => log::LevelFilter::Warn,
    };
    let logger = LOGGER.get_or_init(|| StderrLog(level));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(level);
    }
}

// ------------------------------------------------------------------ setup

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The folders earlier versions of Pyxee used (`dev.pyxee.app` under Local, the former one under Roaming), so the
/// library, the session and the settings carry over. An older version running at the same time would write the same
/// files.
fn data_folders() -> (Option<PathBuf>, Option<PathBuf>) {
    let (local, roaming) = if cfg!(windows) {
        (env_dir("LOCALAPPDATA"), env_dir("APPDATA"))
    } else {
        let share = env_dir("XDG_DATA_HOME")
            .or_else(|| env_dir("HOME").map(|h| h.join(".local").join("share")));
        (share, None)
    };
    (
        roaming.map(|d| d.join(IDENTIFIER)),
        local.map(|d| d.join(IDENTIFIER)),
    )
}

/// yt-dlp: `PYXEE_YTDLP`, next to the program, a `binaries` folder there, then (so a development tree works without
/// setup) the folder `cargo xtask tools` fills in this source tree, then PATH.
fn locate_ytdlp() -> PathBuf {
    let found = crate::tools::locate("yt-dlp", "PYXEE_YTDLP");
    if found.is_file() {
        return found;
    }
    // `cargo xtask tools` saves the program under its sidecar name (`yt-dlp-<triple>.exe`), so look for that as well.
    let names: &[&str] = if cfg!(windows) {
        &["yt-dlp.exe", "yt-dlp-x86_64-pc-windows-msvc.exe"]
    } else {
        &["yt-dlp"]
    };
    let tools = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools");
    names
        .iter()
        .map(|name| tools.join(name))
        .find(|path| path.is_file())
        .unwrap_or(found)
}

fn build() -> App {
    let (old, new) = data_folders();
    let moved = pyxee_shell::datadir::resolve(old.as_deref(), new.as_deref());
    let data_dir = moved.dir;
    if let Some(dir) = &data_dir {
        let _ = std::fs::create_dir_all(dir);
    }
    let env_applied = match data_dir.as_ref().map(|d| d.join(".env")) {
        Some(file) => pyxee_shell::envfile::apply(&file),
        None => Default::default(),
    };
    if let Some(warning) = &env_applied.warning {
        log::warn!("settings: {warning}");
    }
    for note in &moved.notes {
        if note.warn {
            log::warn!("data folder: {}", note.text);
        } else {
            log::info!("data folder: {}", note.text);
        }
    }
    let session_path = data_dir.as_ref().map(|d| d.join("session.json"));
    let player = session_path
        .as_deref()
        .and_then(session::load)
        .map_or_else(|| Player::new(seed()), Player::restore);
    let mut remote = YtDlpSource::new(locate_ytdlp())
        .with_cookies(Cookies::from_env())
        .with_js_runtime(JsRuntime::from_env());
    // The updater's copy of yt-dlp wins over the installed one, unless the developer pinned one with PYXEE_YTDLP.
    if std::env::var_os("PYXEE_YTDLP").is_none_or(|v| v.is_empty()) {
        if let Some(dir) = &data_dir {
            remote = remote.with_preferred(updater::managed_ytdlp_path(dir));
        }
    }
    if !ytmusic::direct_requests_allowed_from_env() {
        remote = remote.without_music_search();
    }
    if !ytmusic::suggestions_allowed_from_env() {
        remote = remote.without_suggestions();
    }
    let source: Arc<dyn MusicSource> = Arc::new(CompositeSource {
        remote,
        local: LocalSource,
    });
    let library = LibraryService::open_or_memory(data_dir.as_ref().map(|d| d.join("library.db")).as_deref());
    let (tx, rx) = channel();
    App {
        player: Mutex::new(player),
        seq: AtomicU64::new(0),
        last_published: Mutex::new(None),
        engine: Mutex::new(tx),
        pending_rx: Mutex::new(Some(rx)),
        volume: Volume::default(),
        library: Mutex::new(library),
        source,
        session_path,
        last_saved: Mutex::new(Vec::new()),
        save_signal: Arc::new(session::SaveSignal::default()),
        on_event: Mutex::new(None),
        sleep: Mutex::new(SleepTimer::new()),
        restore_fade: AtomicBool::new(false),
        audible: Mutex::new(None),
        lyrics: Mutex::new(HashMap::new()),
        cooldown: Mutex::new(Cooldown::default()),
        clock: Instant::now(),
        media: Mutex::new(None),
        sent: Mutex::new(Sent::default()),
        prefs_path: data_dir.as_ref().map(|d| d.join("ui_prefs.json")),
        env: EnvState {
            file: data_dir.as_ref().map(|d| d.join(".env")),
            from_windows: env_applied.from_windows,
        },
        data_dir,
        discord: Mutex::new(None),
    }
}

pub fn start(on_event: EventCallback) {
    init_logging();
    let app = APP.get_or_init(build);
    if let Ok(mut callback) = app.on_event.lock() {
        *callback = Some(on_event);
    }
    if let Ok(mut last) = app.last_published.lock() {
        *last = None; // so that the first thing a new listener gets is the whole state
    }
    SPAWNED.call_once(spawn_threads);
    app.publish(|_| Vec::new());
}

pub fn stop() {
    if let Some(app) = app() {
        if let Ok(mut callback) = app.on_event.lock() {
            *callback = None;
        }
    }
}

fn spawn_threads() {
    let Some(first) = app() else { return };
    let rx = first.pending_rx.lock().ok().and_then(|mut r| r.take());
    if let Some(rx) = rx {
        spawn_runner(rx);
    }
    // Installers that earlier updates downloaded are no longer needed.
    thread::spawn(updater::clear_leftovers);
    // The session is written here, off the thread that reports playback: a burst of requests becomes one write.
    thread::Builder::new()
        .name("session-writer".into())
        .spawn(|| loop {
            let Some(app) = app() else { return };
            if app.save_signal.wait(Duration::from_secs(3600)) {
                thread::sleep(Duration::from_millis(250));
                app.save_session();
            }
        })
        .ok();
    let clock = thread::spawn(|| loop {
        let Some(app) = app() else { return };
        let timed = app.sleep.lock().is_ok_and(|t| t.is_timed());
        if timed {
            thread::sleep(Duration::from_secs(1));
            sleep_tick(app);
        } else {
            thread::park_timeout(Duration::from_secs(300));
        }
    });
    let _ = CLOCK.set(clock.thread().clone());
}

// ------------------------------------------------------------------ events

#[derive(Serialize)]
struct Envelope<'a, T: Serialize> {
    event: &'a str,
    data: T,
}

pub(crate) fn emit<T: Serialize>(event: &str, data: T) {
    let Some(app) = app() else { return };
    let callback = app.on_event.lock().ok().and_then(|c| *c);
    let Some(callback) = callback else { return };
    let Ok(json) = serde_json::to_string(&Envelope { event, data }) else {
        return;
    };
    let Ok(text) = CString::new(json) else { return };
    callback(text.into_raw());
}

fn publish(f: impl FnOnce(&mut Player) -> Vec<Effect>) {
    if let Some(app) = app() {
        app.publish(f);
    }
}

impl App {
    /// Runs `f` against the player, forwards the resulting effects to the engine (or saves the session for
    /// `Persist`), and pushes the new state to the listener: the whole state, or just the position when that is all
    /// that moved (the queue, with every title and thumbnail, need not travel four times a second).
    fn publish(&self, f: impl FnOnce(&mut Player) -> Vec<Effect>) {
        // Effects go to the engine while the player is still locked, so that two threads publishing at once reach it
        // in the order their changes were made. Lock order is always player, then engine.
        let Ok(mut player) = self.player.lock() else { return };
        let effects = f(&mut player);
        let mut dto = state_dto(&player);
        dto.seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let mut save = false;
        if let Ok(tx) = self.engine.lock() {
            for effect in effects {
                if effect == Effect::Persist {
                    save = true;
                } else {
                    let _ = tx.send(effect);
                }
            }
        }
        let mut key = dto.clone();
        key.seq = 0;
        key.position_ms = 0;
        let only_position = match self.last_published.lock() {
            Ok(mut last) => {
                let same = last.as_ref() == Some(&key);
                *last = Some(key);
                same
            }
            Err(_) => false,
        };
        drop(player);
        if save {
            self.save_signal.request();
        }
        self.on_state(&dto);
        if only_position {
            emit(
                "position",
                PositionDto {
                    seq: dto.seq,
                    position_ms: dto.position_ms,
                },
            );
        } else {
            emit("state", dto);
        }
    }

    fn save_session(&self) {
        let Some(path) = &self.session_path else { return };
        let persisted = match self.player.lock() {
            Ok(p) => p.persisted(),
            Err(_) => return,
        };
        let Ok(mut last) = self.last_saved.lock() else { return };
        if let Err(e) = session::save_if_changed(path, &persisted, &mut last) {
            log::warn!("could not save session: {e}");
        }
    }

    /// After every state change: keeps the system transport tile in step. Cheap: it only touches the OS when
    /// something visible changed.
    fn on_state(&self, s: &PlayerStateDto) {
        let entry = s.current.and_then(|id| s.queue.iter().find(|e| e.id == id));
        let phase = match &s.transport {
            crate::dto::TransportDto::Idle => Phase::Idle,
            crate::dto::TransportDto::Loading { .. } | crate::dto::TransportDto::Stalled { .. } => Phase::Loading,
            crate::dto::TransportDto::Playing { .. } => Phase::Playing,
            crate::dto::TransportDto::Paused { .. } => Phase::Paused,
            crate::dto::TransportDto::Failed { .. } => Phase::Failed,
        };
        if phase == Phase::Playing && self.restore_fade.swap(false, Ordering::Relaxed) {
            self.volume.set_fade(1.0);
        }
        // Discord decides for itself (and cheaply) whether anything changed enough to be worth sending.
        if let Ok(guard) = self.discord.lock() {
            if let Some(discord) = guard.as_ref() {
                discord.on_state(phase, entry.map(|e| &e.track), s.position_ms);
            }
        }
        let Ok(mut sent) = self.sent.lock() else { return };
        let Ok(mut media) = self.media.lock() else { return };
        let Some(controls) = media.as_mut() else { return };
        let track_key: TrackKey = entry.map(|e| (e.id, e.track.thumbnail.clone()));
        if sent.track != track_key {
            sent.track.clone_from(&track_key);
            if let Some(e) = entry {
                let duration = e.track.duration_ms.map(Duration::from_millis);
                let result = controls.set_metadata(MediaMetadata {
                    title: Some(&e.track.title),
                    artist: Some(&e.track.artist),
                    album: None,
                    cover_url: e.track.thumbnail.as_deref(),
                    duration,
                });
                if let Err(e) = result {
                    log::debug!("media metadata not set: {e:?}");
                }
            }
            sent.playback = None; // a new song: send its state afresh
        }
        let key = (
            match phase {
                Phase::Idle => 0,
                Phase::Playing => 1,
                Phase::Paused => 2,
                _ => 3,
            },
            s.position_ms / 1000,
        );
        if sent.playback != Some(key) {
            sent.playback = Some(key);
            let has_track = entry.is_some();
            let progress = has_track.then(|| MediaPosition(Duration::from_millis(s.position_ms)));
            let state = match phase {
                Phase::Playing => MediaPlayback::Playing { progress },
                Phase::Paused | Phase::Loading | Phase::Failed if has_track => MediaPlayback::Paused { progress },
                _ => MediaPlayback::Stopped,
            };
            if let Err(e) = controls.set_playback(state) {
                log::debug!("media playback state not set: {e:?}");
            }
        }
    }
}

// ------------------------------------------------------------------ media keys

fn seek_target(position_ms: u64, dir: SeekDirection, by_ms: u64) -> u64 {
    match dir {
        SeekDirection::Forward => position_ms.saturating_add(by_ms),
        SeekDirection::Backward => position_ms.saturating_sub(by_ms),
    }
}

fn on_media_event(event: MediaControlEvent) {
    let Some(app) = app() else { return };
    let position = || app.player.lock().map_or(0, |p| state_dto(&p).position_ms);
    let cmd = match event {
        MediaControlEvent::Play => Command::Resume,
        MediaControlEvent::Pause | MediaControlEvent::Stop => Command::Pause,
        MediaControlEvent::Toggle => Command::Toggle,
        MediaControlEvent::Next => Command::Next,
        MediaControlEvent::Previous => Command::Previous,
        MediaControlEvent::Seek(dir) => Command::Seek(seek_target(position(), dir, 10_000)),
        MediaControlEvent::SeekBy(dir, by) => Command::Seek(seek_target(position(), dir, by.as_millis() as u64)),
        MediaControlEvent::SetPosition(MediaPosition(to)) => Command::Seek(to.as_millis() as u64),
        // MPRIS `Raise` / `Quit`: clicking the player in a KDE or GNOME media widget.
        MediaControlEvent::Raise => return emit("window", json!({ "action": "show" })),
        MediaControlEvent::Quit => return emit("window", json!({ "action": "quit" })),
        _ => return,
    };
    app.publish(|p| p.command(cmd));
}

/// Windows' System Media Transport Controls need the window handle, which only the shell knows; Linux (MPRIS) does
/// not. Called once the window exists.
fn attach_media(app: &App, hwnd: i64) {
    let Ok(mut slot) = app.media.lock() else { return };
    if slot.is_some() {
        return;
    }
    let hwnd = (hwnd != 0).then_some(hwnd as usize as *mut std::ffi::c_void);
    let config = PlatformConfig {
        dbus_name: IDENTIFIER,
        display_name: "Pyxee",
        desktop_entry: Some("pyxee"),
        hwnd,
    };
    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("media keys are not available: {e:?}");
            return;
        }
    };
    if let Err(e) = controls.attach(on_media_event) {
        log::warn!("could not attach the media key handler: {e:?}");
        return;
    }
    *slot = Some(controls);
    drop(slot);
    if let Ok(mut sent) = app.sent.lock() {
        *sent = Sent::default(); // the next state publishes the song and its state to the new tile
    }
    app.publish(|_| Vec::new());
}

// ------------------------------------------------------------------ sleep timer

fn emit_sleep(app: &App) {
    let status = app
        .sleep
        .lock()
        .ok()
        .and_then(|t| t.status(now_ms().max(0) as u64))
        .map(SleepStatusDto::from);
    emit("sleep", SleepStateDto { status });
}

fn sync_stop_flag(app: &App) {
    let flag = app.sleep.lock().is_ok_and(|t| t.stop_after_current());
    app.publish(|p| p.command(Command::StopAfterCurrent(flag)));
}

fn wake_clock() {
    if let Some(clock) = CLOCK.get() {
        clock.unpark();
    }
}

fn sleep_tick(app: &App) {
    let tick = match app.sleep.lock() {
        Ok(mut t) => t.tick(now_ms().max(0) as u64),
        Err(_) => return,
    };
    let Some(t) = tick else { return };
    app.volume.set_fade(t.volume_factor);
    if t.fire {
        log::info!("sleep timer: time is up, pausing");
        app.publish(|p| p.command(Command::Pause));
        // Stay silent until playback is started again, so nothing blips back in before the pause lands.
        app.restore_fade.store(true, Ordering::Relaxed);
    }
    emit_sleep(app);
}

fn sleep_set(app: &App, setting: SleepSetting) {
    if let Ok(mut t) = app.sleep.lock() {
        t.set(setting, now_ms().max(0) as u64);
    }
    wake_clock();
    app.volume.set_fade(1.0);
    sync_stop_flag(app);
    emit_sleep(app);
}

fn sleep_cancel(app: &App) {
    if let Ok(mut t) = app.sleep.lock() {
        t.cancel();
    }
    app.volume.set_fade(1.0);
    sync_stop_flag(app);
    emit_sleep(app);
}

fn sleep_extend(app: &App, minutes: u32) {
    if let Ok(mut t) = app.sleep.lock() {
        t.extend_minutes(minutes, now_ms().max(0) as u64);
    }
    wake_clock();
    app.volume.set_fade(1.0);
    sync_stop_flag(app);
    emit_sleep(app);
}

/// Engine events seen after the player has processed them: a song ended (sleep timer).
fn on_engine_event(app: &App, e: &EngineEvent) {
    match e {
        EngineEvent::Audible { id } => {
            if let Ok(mut a) = app.audible.lock() {
                *a = Some(id.0);
            }
        }
        EngineEvent::Ended { id } => {
            let genuine = app.audible.lock().is_ok_and(|mut a| a.take() == Some(id.0));
            if genuine {
                let done = app.sleep.lock().is_ok_and(|mut t| t.on_track_finished());
                if done {
                    log::info!("sleep timer: last song finished, staying paused");
                }
                sync_stop_flag(app);
                emit_sleep(app);
            }
        }
        _ => {}
    }
}

// ------------------------------------------------------------------ the engine thread

struct BridgeHost {
    recorder: Mutex<HistoryRecorder>,
}

impl BridgeHost {
    /// Feeds listening into the history: what became audible, and how long it has been heard.
    fn record(&self, app: &App, e: &EngineEvent) {
        let Ok(mut rec) = self.recorder.lock() else { return };
        let track_of = |id: pyxee_core::EntryId| {
            app.player
                .lock()
                .ok()
                .and_then(|p| p.queue().get(id).map(|entry| entry.track.clone()))
        };
        match e {
            EngineEvent::Audible { id } => {
                if let Some(track) = track_of(*id) {
                    rec.audible(*id, track);
                }
            }
            EngineEvent::Position { id, ms } => {
                if let Some(track) = rec.position(*id, *ms) {
                    drop(rec);
                    if let Err(err) = with_library(|l| l.record_play(&track, now_ms())) {
                        log::warn!("could not record play: {}", err.message);
                    }
                }
            }
            EngineEvent::TrackDetails { id, .. } => {
                drop(rec);
                if let Some(track) = track_of(*id) {
                    if let Err(err) = with_library(|l| l.refresh_track(&track)) {
                        log::warn!("could not update a saved song: {}", err.message);
                    }
                }
            }
            _ => {}
        }
    }
}

impl Host for BridgeHost {
    fn engine_event(&self, e: EngineEvent) {
        let Some(app) = app() else { return };
        app.publish(|p| p.engine(e.clone()));
        self.record(app, &e);
        on_engine_event(app, &e);
    }

    fn notice(&self, message: String) {
        log::info!("notice shown to the listener: {message}");
        emit("notice", NoticeDto { message });
    }

    fn command(&self, c: Command) {
        publish(|p| p.command(c));
    }
}

fn watched(d: &DeviceIdent) -> DeviceRef {
    DeviceRef {
        id: d.id.clone(),
        name: d.name.clone(),
    }
}

/// Opens the default output device and starts the stream on it, pulling from `engine`. When the device runs at
/// another sample rate than the engine was built for, the engine is rebuilt at the new rate first.
fn open_output(engine: &mut Engine, errors: &Sender<String>) -> Result<Output, AudioError> {
    let device = DeviceInfo::default_output()?;
    engine.change_rate(device.rate());
    let tx = errors.clone();
    device.start_with(engine.consumer(), move |e| {
        let _ = tx.send(e);
    })
}

fn spawn_runner(rx: Receiver<Effect>) {
    thread::Builder::new()
        .name("engine".into())
        .spawn(move || run_engine(rx))
        .ok();
}

fn run_engine(rx: Receiver<Effect>) {
    let Some(app) = app() else { return };
    let host = Arc::new(BridgeHost {
        recorder: Mutex::new(HistoryRecorder::default()),
    });
    let first = DeviceInfo::default_output();
    let rate = first.as_ref().map_or(FALLBACK_RATE, DeviceInfo::rate);
    let mut engine = Engine::new(host.clone(), Arc::clone(&app.source), rate, app.volume.clone());
    let (err_tx, err_rx) = channel::<String>();
    let clock = Instant::now();
    let since_start = || clock.elapsed().as_millis() as u64;
    // `output` keeps the audio stream alive on this thread. `None` while there is no usable device.
    let mut output: Option<Output> = None;
    let mut watch = DeviceWatch::new(None, 0);
    match first.and_then(|d| {
        let tx = err_tx.clone();
        d.start_with(engine.consumer(), move |e| {
            let _ = tx.send(e);
        })
    }) {
        Ok(o) => {
            watch = DeviceWatch::new(Some(watched(o.ident())), since_start());
            output = Some(o);
        }
        Err(e) => log::warn!("audio output unavailable at start: {e}"),
    }
    let mut last_save = Instant::now();
    let mut last_check = Instant::now();
    loop {
        match rx.recv_timeout(engine.poll_interval()) {
            Ok(effect) => {
                if matches!(
                    effect,
                    Effect::Resume | Effect::StartTrack { .. } | Effect::Stop | Effect::Seek { .. }
                ) {
                    watch.on_user(UserEffect::TookOver);
                }
                match effect {
                    // No device: say so instead of playing into the void. Retry works once one is back.
                    Effect::StartTrack { id, .. } if watch.is_lost() => {
                        host.engine_event(EngineEvent::LoadFailed {
                            id,
                            failure: Failure::Other("No audio output device. Connect one and press play again.".into()),
                        })
                    }
                    effect => engine.handle(effect),
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        engine.poll();
        let mut todo: VecDeque<DeviceAction> = VecDeque::new();
        while let Ok(e) = err_rx.try_recv() {
            todo.extend(watch.on_stream_error(&e, engine.is_playing()));
        }
        if last_check.elapsed() >= DEVICE_CHECK_EVERY {
            last_check = Instant::now();
            todo.extend(watch.tick(&Observation {
                now_ms: since_start(),
                default_device: DeviceInfo::default_ident().as_ref().map(watched),
                callbacks: output.as_ref().map_or(0, Output::callbacks),
                playing: engine.is_playing(),
            }));
        }
        while let Some(action) = todo.pop_front() {
            match action {
                DeviceAction::Rebuild { reason } => {
                    output = None; // the old stream goes first
                    match open_output(&mut engine, &err_tx) {
                        Ok(o) => {
                            while err_rx.try_recv().is_ok() {}
                            todo.extend(watch.rebuilt(&watched(o.ident()), o.rate(), since_start()));
                            output = Some(o);
                        }
                        Err(e) => {
                            log::debug!("audio output rebuild ({reason}) failed: {e}");
                            todo.extend(watch.rebuild_failed(&e.to_string(), since_start(), engine.is_playing()));
                        }
                    }
                }
                DeviceAction::PausePlayer => host.command(Command::Pause),
                DeviceAction::ResumePlayer => host.command(Command::Resume),
                DeviceAction::Notice(message) => host.notice(message),
                DeviceAction::Log { warn: true, text } => log::warn!("{text}"),
                DeviceAction::Log { warn: false, text } => log::info!("{text}"),
            }
        }
        // The periodic save is for the playing position; paused or idle, a pause, skip or queue change saves itself.
        if last_save.elapsed() >= SAVE_EVERY {
            last_save = Instant::now();
            if engine.is_playing() {
                app.save_signal.request();
            }
        }
    }
}

// ------------------------------------------------------------------ requests

pub struct CallError {
    message: String,
    kind: Option<String>,
}

impl From<String> for CallError {
    fn from(message: String) -> Self {
        Self { message, kind: None }
    }
}

impl From<&str> for CallError {
    fn from(message: &str) -> Self {
        Self::from(message.to_string())
    }
}

impl From<SearchErrorDto> for CallError {
    fn from(e: SearchErrorDto) -> Self {
        let kind = serde_json::to_value(&e.kind).ok().and_then(|v| v.as_str().map(str::to_owned));
        Self {
            message: e.message,
            kind,
        }
    }
}

impl From<pyxee_sources::SourceError> for CallError {
    fn from(e: pyxee_sources::SourceError) -> Self {
        SearchErrorDto::from(&e).into()
    }
}

#[derive(Deserialize)]
struct Request {
    method: String,
    #[serde(default)]
    params: Value,
}

pub fn failure(error: &str) -> String {
    json!({ "ok": false, "error": error }).to_string()
}

pub fn call(request: &str) -> String {
    let outcome = catch_unwind(AssertUnwindSafe(|| handle_text(request)))
        .unwrap_or_else(|_| Err(CallError::from("the bridge panicked")));
    match outcome {
        Ok(data) => json!({ "ok": true, "data": data }),
        Err(e) => json!({ "ok": false, "error": e.message, "kind": e.kind }),
    }
    .to_string()
}

/// Runs the request on its own thread and reports the reply as a `reply` event.
pub fn call_async(request: Option<String>, id: u64) {
    thread::spawn(move || {
        let reply = match request {
            Some(text) => call(&text),
            None => failure("the request is not UTF-8"),
        };
        let value: Value = serde_json::from_str(&reply).unwrap_or(Value::Null);
        emit("reply", json!({ "id": id, "reply": value }));
    });
}

/// Makes Discord Rich Presence match the setting: starts it when it is on (and this build has a Discord application ID),
/// clears it and stops when it is off.
fn sync_discord(app: &App, on: bool) {
    let stopped = {
        let Ok(mut slot) = app.discord.lock() else { return };
        if on {
            if slot.is_none() {
                *slot = discord::init();
            }
            None
        } else {
            slot.take()
        }
    };
    // Outside the lock: clearing waits for Discord (a second at most) and `on_state` must not be held up by it.
    if let Some(discord) = stopped {
        discord.shutdown();
    } else if on {
        app.publish(|_| Vec::new()); // show the song that is playing now
    }
}

fn handle_text(request: &str) -> Result<Value, CallError> {
    let request: Request = serde_json::from_str(request).map_err(|e| CallError::from(format!("bad request: {e}")))?;
    let app = app().ok_or("the bridge is not started")?;
    handle(app, &request.method, &request.params)
}

fn arg<T: DeserializeOwned>(params: &Value, key: &str) -> Result<T, CallError> {
    let value = params.get(key).cloned().unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|e| CallError::from(format!("bad `{key}`: {e}")))
}

fn json_of<T: Serialize>(value: T) -> Result<Value, CallError> {
    serde_json::to_value(value).map_err(|e| CallError::from(e.to_string()))
}

fn read_library<T>(f: impl FnOnce(&LibraryService) -> Result<T, String>) -> Result<T, CallError> {
    let app = app().ok_or("the bridge is not started")?;
    let lib = app.library.lock().map_err(|_| CallError::from("The library is unavailable."))?;
    f(&lib).map_err(CallError::from)
}

/// Runs a library change; on success tells the listener the library changed.
fn with_library<T>(f: impl FnOnce(&mut LibraryService) -> Result<T, String>) -> Result<T, CallError> {
    let app = app().ok_or("the bridge is not started")?;
    let (result, snapshot) = {
        let mut lib = app.library.lock().map_err(|_| CallError::from("The library is unavailable."))?;
        let result = f(&mut lib);
        let snapshot = if result.is_ok() { lib.snapshot().ok() } else { None };
        (result, snapshot)
    };
    if let Some(snapshot) = snapshot {
        emit("library", snapshot);
    }
    result.map_err(CallError::from)
}

fn handle(app: &App, method: &str, params: &Value) -> Result<Value, CallError> {
    match method {
        "getState" => {
            let player = app.player.lock().map_err(|_| CallError::from("player state unavailable"))?;
            let mut dto = state_dto(&player);
            dto.seq = app.seq.load(Ordering::SeqCst);
            json_of(dto)
        }
        "dispatch" => {
            let command: CommandDto =
                serde_json::from_value(params.clone()).map_err(|e| CallError::from(format!("bad command: {e}")))?;
            app.publish(|player| player.command(command.into()));
            Ok(Value::Null)
        }
        "setVolume" => {
            let percent: u8 = arg(params, "percent")?;
            app.volume.set_percent(percent);
            Ok(Value::Null)
        }
        "quit" => {
            app.save_session();
            sync_discord(app, false); // clears the presence, waiting a second at most
            Ok(Value::Null)
        }
        "attachMedia" => {
            let hwnd: i64 = arg(params, "hwnd")?;
            attach_media(app, hwnd);
            Ok(Value::Null)
        }
        "prefsGet" => Ok(app
            .prefs_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map_or(Value::Null, Value::String)),
        "prefsSet" => {
            let text: String = arg(params, "json")?;
            if text.len() > PREFS_MAX_BYTES {
                return Err("the settings are too large".into());
            }
            if let Some(path) = &app.prefs_path {
                pyxee_shell::atomic::write(path, text.as_bytes())
                    .map_err(|e| CallError::from(format!("could not save the settings: {e}")))?;
            }
            Ok(Value::Null)
        }
        // ------------------------------------------------- advanced, updates, discord
        "envGet" => json_of(advanced::env_get(&app.env)?),
        "envSet" => {
            let vars: Vec<crate::dto::EnvVarDto> = arg(params, "vars")?;
            json_of(advanced::env_set(&app.env, vars)?)
        }
        "updateCheckApp" => json_of(updater::update_check_app(arg(params, "prereleases")?)?),
        "updateInstallApp" => {
            updater::update_install_app(arg(params, "prereleases")?)?;
            // The installer is running: close the window so it can replace the files.
            emit("window", json!({ "action": "quit" }));
            Ok(Value::Null)
        }
        "updateCheckYtdlp" => json_of(updater::update_check_ytdlp(app.data_dir.as_deref())?),
        "updateInstallYtdlp" => json_of(updater::update_install_ytdlp(app.data_dir.as_deref())?),
        "openReleasePage" => {
            opener::open_url(opener::RELEASES_URL)?;
            Ok(Value::Null)
        }
        "discordSet" => {
            sync_discord(app, arg(params, "on")?);
            Ok(Value::Null)
        }
        // -------------------------------------------------------------- sources
        "search" => {
            let query: String = arg(params, "query")?;
            let filter: SearchFilterDto = arg(params, "filter")?;
            let items = app.source.search(&query, filter.into())?;
            json_of(SearchResponseDto {
                items: items.iter().map(SearchItemDto::from).collect(),
            })
        }
        "suggest" => {
            let prefix: String = arg(params, "prefix")?;
            json_of(app.source.suggest(&prefix).unwrap_or_default())
        }
        "related" => {
            let key: String = arg(params, "key")?;
            let count = arg::<u32>(params, "count")?.clamp(1, 30) as usize;
            let tracks = pyxee_sources::gate::background(|| app.source.related(&key, count))?;
            json_of(tracks.iter().map(TrackDto::from).collect::<Vec<_>>())
        }
        "explore" => {
            let shelves = pyxee_sources::gate::background(|| app.source.explore())?;
            json_of(shelves.iter().map(ExploreShelfDto::from).collect::<Vec<_>>())
        }
        "genres" => {
            let genres = pyxee_sources::gate::background(|| app.source.genres())?;
            json_of(genres.iter().map(GenreDto::from).collect::<Vec<_>>())
        }
        "genre" => {
            let id: String = arg(params, "params")?;
            let shelves = app.source.genre(&id)?;
            json_of(shelves.iter().map(ExploreShelfDto::from).collect::<Vec<_>>())
        }
        "playlistTracks" => {
            let id: String = arg(params, "id")?;
            let tracks = app.source.playlist_tracks(&id)?;
            json_of(tracks.iter().map(TrackDto::from).collect::<Vec<_>>())
        }
        "searchFilters" => {
            use pyxee_sources::SearchFilter as F;
            let supported = app.source.supported_filters();
            json_of(
                [F::All, F::Songs, F::Videos, F::Albums, F::Artists, F::Playlists]
                    .into_iter()
                    .map(|f| FilterInfoDto {
                        filter: f.into(),
                        supported: supported.contains(&f),
                    })
                    .collect::<Vec<_>>(),
            )
        }
        "lyrics" => json_of(lyrics(app, params)?),
        // -------------------------------------------------------------- library
        "getLibrary" => json_of(read_library(|l| l.snapshot())?),
        "setLiked" => {
            let track: TrackDto = arg(params, "track")?;
            let liked: bool = arg(params, "liked")?;
            with_library(|l| l.set_liked(&track, liked, now_ms()))?;
            Ok(Value::Null)
        }
        "liked" => json_of(read_library(|l| l.liked())?),
        "playlistCreate" => {
            let name: String = arg(params, "name")?;
            let tracks: Vec<TrackDto> = arg(params, "tracks")?;
            json_of(with_library(|l| l.create_playlist(&name, &tracks, now_ms()))?)
        }
        "playlistRename" => {
            let id: i64 = arg(params, "id")?;
            let name: String = arg(params, "name")?;
            with_library(|l| l.rename_playlist(id, &name, now_ms()))?;
            Ok(Value::Null)
        }
        "playlistDelete" => {
            let id: i64 = arg(params, "id")?;
            json_of(with_library(|l| l.delete_playlist(id))?)
        }
        "playlistRestore" => {
            let snapshot: crate::dto::PlaylistSnapshotDto = arg(params, "snapshot")?;
            with_library(|l| l.restore_playlist(&snapshot))?;
            Ok(Value::Null)
        }
        "playlistAdd" => {
            let id: i64 = arg(params, "id")?;
            let tracks: Vec<TrackDto> = arg(params, "tracks")?;
            json_of(with_library(|l| l.add_to_playlist(id, &tracks, now_ms()))?)
        }
        "playlistGet" => {
            let id: i64 = arg(params, "id")?;
            json_of(read_library(|l| l.playlist(id))?)
        }
        "playlistRemoveEntry" => {
            let entry_id: i64 = arg(params, "entryId")?;
            json_of(with_library(|l| l.remove_entry(entry_id, now_ms()))?)
        }
        "playlistRestoreEntry" => {
            let removed: crate::dto::RemovedEntryDto = arg(params, "removed")?;
            json_of(with_library(|l| l.restore_entry(&removed, now_ms()))?)
        }
        "playlistMoveEntry" => {
            let entry_id: i64 = arg(params, "entryId")?;
            let to: usize = arg(params, "to")?;
            with_library(|l| l.move_entry(entry_id, to, now_ms()))?;
            Ok(Value::Null)
        }
        "historyGet" => {
            let limit: usize = arg(params, "limit")?;
            json_of(read_library(|l| l.history(limit))?)
        }
        "historyClear" => json_of(with_library(|l| l.clear_history())?),
        "historyRestore" => {
            let items: Vec<crate::dto::HistoryItemDto> = arg(params, "items")?;
            with_library(|l| l.restore_history(&items))?;
            Ok(Value::Null)
        }
        "home" => json_of(read_library(|l| l.home(now_ms()))?),
        // -------------------------------------------------------------- sleep timer
        "sleepSet" => {
            let setting: SleepSettingDto = arg(params, "setting")?;
            sleep_set(app, setting.into());
            Ok(Value::Null)
        }
        "sleepCancel" => {
            sleep_cancel(app);
            Ok(Value::Null)
        }
        "sleepExtend" => {
            let minutes: u32 = arg(params, "minutes")?;
            sleep_extend(app, minutes);
            Ok(Value::Null)
        }
        "sleepGet" => {
            let status = app
                .sleep
                .lock()
                .ok()
                .and_then(|t| t.status(now_ms().max(0) as u64))
                .map(SleepStatusDto::from);
            json_of(SleepStateDto { status })
        }
        other => Err(CallError::from(format!("unknown method: {other}"))),
    }
}

/// Synced lyrics for a song, from LRCLIB. Answers are kept for the session so flipping tabs never asks twice.
fn lyrics(app: &App, params: &Value) -> Result<LyricsDto, CallError> {
    let key: String = arg(params, "key")?;
    let title: String = arg(params, "title")?;
    let artist: String = arg(params, "artist")?;
    let duration_ms: Option<u64> = arg(params, "durationMs")?;
    if let Some(hit) = app.lyrics.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Ok(hit.into());
    }
    let now = || u64::try_from(app.clock.elapsed().as_millis()).unwrap_or(u64::MAX);
    // A service that just refused the app is left alone, so flipping songs cannot hammer a block.
    let waiting = app.cooldown.lock().ok().and_then(|c| c.remaining_secs(now()));
    if let Some(secs) = waiting {
        return Ok(LyricsDto::Error {
            message: format!("the lyrics service asked the app to wait, try again in about {secs} s"),
        });
    }
    let query = Query::new(&title, &artist, duration_ms);
    match fetch(LRCLIB, &query) {
        Ok(found) => {
            if let Ok(mut cache) = app.lyrics.lock() {
                if cache.len() >= LYRICS_CACHE_MAX {
                    cache.clear();
                }
                cache.insert(key, found.clone());
            }
            Ok(found.into())
        }
        Err(e) => {
            log::info!("lyrics lookup failed: {e}");
            if let Ok(mut c) = app.cooldown.lock() {
                c.record(&e, now());
            }
            Ok(LyricsDto::Error { message: e.to_string() })
        }
    }
}
