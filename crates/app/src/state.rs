//! Shared app state and the one place that turns player changes into UI events and engine work.

use crate::dto::{state_dto, BackdropDto};
use crate::library_service::LibraryService;
use crate::session;
use pyxee_core::{Effect, Player};
use pyxee_sources::MusicSource;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

pub const STATE_EVENT: &str = "player-state";
pub const LIBRARY_EVENT: &str = "library-changed";
pub const NOTICE_EVENT: &str = "notice";

pub struct AppState {
    pub log: crate::logging::LogInfo,
    pub log_error: Option<String>,
    pub volume: pyxee_audio::pipeline::Volume,
    pub library: Mutex<LibraryService>,
    pub source: Arc<dyn MusicSource>,
    pub player: Mutex<Player>,
    pub engine: Mutex<Sender<Effect>>,
    pub backdrop: BackdropDto,
    pub session_path: Option<PathBuf>,
}

impl AppState {
    pub fn save_session(&self) {
        let Some(path) = &self.session_path else {
            return;
        };
        let persisted = match self.player.lock() {
            Ok(p) => p.persisted(),
            Err(_) => return,
        };
        if let Err(e) = session::save(path, &persisted) {
            log::warn!("could not save session: {e}");
        }
    }
}

/// Runs `f` against the player, forwards the resulting effects to the engine (or saves the
/// session for `Persist`), and pushes the new state to the UI.
pub fn publish(app: &AppHandle, f: impl FnOnce(&mut Player) -> Vec<Effect>) {
    let state = app.state::<AppState>();
    let (effects, dto) = match state.player.lock() {
        Ok(mut p) => {
            let fx = f(&mut p);
            (fx, state_dto(&p))
        }
        Err(_) => return, // a panic elsewhere poisoned the lock; nothing sensible to do
    };
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
    if save {
        state.save_session();
    }
    crate::shell::on_state(app, &dto);
    if let Err(e) = app.emit(STATE_EVENT, dto) {
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
            if let Err(e) = app.emit(LIBRARY_EVENT, s) {
                log::warn!("could not emit library state: {e}");
            }
        }
        Err(e) => log::warn!("could not read the library: {e}"),
    }
}
