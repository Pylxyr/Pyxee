//! Shared app state and the one place that turns player changes into UI events and engine work.

use crate::dto::{state_dto, BackdropDto, PlayerStateDto, PositionDto};
use crate::library_service::LibraryService;
use crate::session;
use pyxee_core::{Effect, Player};
use pyxee_sources::MusicSource;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

pub const STATE_EVENT: &str = "player-state";
/// Carries only `seq` and the position while nothing else about the player changed (see `publish`).
pub const POSITION_EVENT: &str = "player-position";
pub const LIBRARY_EVENT: &str = "library-changed";
pub const NOTICE_EVENT: &str = "notice";

pub struct AppState {
    pub log: crate::logging::LogInfo,
    pub log_error: Option<String>,
    pub volume: pyxee_audio::pipeline::Volume,
    pub library: Mutex<LibraryService>,
    pub source: Arc<dyn MusicSource>,
    pub player: Mutex<Player>,
    /// Number of the last state published; only changed while `player` is locked.
    pub seq: AtomicU64,
    /// The last state published, with `seq` and the position zeroed: what a new state is compared with to see whether
    /// only the position moved. Only changed while `player` is locked.
    pub last_published: Mutex<Option<PlayerStateDto>>,
    pub engine: Mutex<Sender<Effect>>,
    pub backdrop: BackdropDto,
    pub session_path: Option<PathBuf>,
    /// The bytes of the session file as last written, so an unchanged session is not written again.
    pub last_saved: Mutex<Vec<u8>>,
    /// Wakes the thread that writes the session (see `request_save`).
    pub save_signal: Arc<session::SaveSignal>,
}

impl AppState {
    /// Asks for the session to be saved soon, by the writer thread: nothing here waits for the disk, which is what
    /// the thread that reports playback must not do (a write and flush can take a long time while an antivirus
    /// scans it). Use `save_session` where it has to be on disk when the call returns (quitting).
    pub fn request_save(&self) {
        self.save_signal.request();
    }

    /// Saves the session now, on this thread.
    pub fn save_session(&self) {
        let Some(path) = &self.session_path else {
            return;
        };
        let persisted = match self.player.lock() {
            Ok(p) => p.persisted(),
            Err(_) => return,
        };
        let Ok(mut last) = self.last_saved.lock() else {
            return;
        };
        if let Err(e) = session::save_if_changed(path, &persisted, &mut last) {
            log::warn!("could not save session: {e}");
        }
    }
}

/// Runs `f` against the player, forwards the resulting effects to the engine (or saves the
/// session for `Persist`), and pushes the new state to the UI.
pub fn publish(app: &AppHandle, f: impl FnOnce(&mut Player) -> Vec<Effect>) {
    let state = app.state::<AppState>();
    // Effects go to the engine while the player is still locked: two threads publishing at once must reach the
    // engine in the order their changes were made to the player (a Pause must not overtake the Resume before it).
    // Lock order is always player, then engine; nothing locks them the other way round.
    let Ok(mut p) = state.player.lock() else {
        return; // a panic elsewhere poisoned the lock; nothing sensible to do
    };
    let effects = f(&mut p);
    let mut dto = state_dto(&p);
    dto.seq = state.seq.fetch_add(1, Ordering::SeqCst) + 1;
    let mut save = false;
    if let Ok(tx) = state.engine.lock() {
        for e in effects {
            if e == Effect::Persist {
                save = true;
            } else {
                let _ = tx.send(e);
            }
        }
    }
    // Only the position moved (the 4 Hz tick): the queue, with every title and thumbnail, does not need to travel.
    let mut key = dto.clone();
    key.seq = 0;
    key.position_ms = 0;
    let only_position = match state.last_published.lock() {
        Ok(mut last) => {
            let same = last.as_ref() == Some(&key);
            *last = Some(key);
            same
        }
        Err(_) => false,
    };
    drop(p);
    if save {
        state.request_save();
    }
    crate::shell::on_state(app, &dto);
    let sent = if only_position {
        let light = PositionDto {
            seq: dto.seq,
            position_ms: dto.position_ms,
        };
        crate::shell::emit_awake(app, POSITION_EVENT, light)
    } else {
        crate::shell::emit_awake(app, STATE_EVENT, dto)
    };
    if let Err(e) = sent {
        log::warn!("could not emit state: {e}");
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Runs a library change; on success tells the UI the library changed.
pub fn with_library<T>(
    app: &AppHandle,
    f: impl FnOnce(&mut LibraryService) -> Result<T, String>,
) -> Result<T, String> {
    let state = app.state::<AppState>();
    let result = {
        let mut lib = state
            .library
            .lock()
            .map_err(|_| "The library is unavailable.".to_string())?;
        f(&mut lib)
    };
    if result.is_ok() {
        emit_library(app);
    }
    result
}

pub fn emit_library(app: &AppHandle) {
    let state = app.state::<AppState>();
    let snapshot = match state.library.lock() {
        Ok(lib) => lib.snapshot(),
        Err(_) => return,
    };
    match snapshot {
        Ok(s) => {
            if let Err(e) = crate::shell::emit_awake(app, LIBRARY_EVENT, s) {
                log::warn!("could not emit library state: {e}");
            }
        }
        Err(e) => log::warn!("could not read the library: {e}"),
    }
}
