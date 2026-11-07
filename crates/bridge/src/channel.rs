//! The player behind the C interface: requests are applied to it, and every change is pushed to the listener.

use crate::dto::{state_dto, CommandDto, PlayerStateDto, PositionDto};
use pyxee_core::{Effect, Player};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::ffi::{c_char, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

pub type EventCallback = extern "C" fn(*mut c_char);

struct Bridge {
    player: Mutex<Player>,
    /// Order of publication, as in the Tauri shell: the UI keeps the state with the highest one.
    seq: AtomicU64,
    last_published: Mutex<Option<PlayerStateDto>>,
    on_event: Mutex<Option<EventCallback>>,
}

static BRIDGE: OnceLock<Bridge> = OnceLock::new();

fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64)
}

pub fn start(on_event: EventCallback) {
    let bridge = BRIDGE.get_or_init(|| Bridge {
        player: Mutex::new(Player::new(seed())),
        seq: AtomicU64::new(0),
        last_published: Mutex::new(None),
        on_event: Mutex::new(None),
    });
    if let Ok(mut callback) = bridge.on_event.lock() {
        *callback = Some(on_event);
    }
    if let Ok(mut last) = bridge.last_published.lock() {
        *last = None; // so that the first thing the new listener gets is the whole state
    }
    publish(|_| Vec::new());
}

pub fn stop() {
    if let Some(bridge) = BRIDGE.get() {
        if let Ok(mut callback) = bridge.on_event.lock() {
            *callback = None;
        }
    }
}

#[derive(Serialize)]
struct Envelope<'a, T: Serialize> {
    event: &'a str,
    data: T,
}

fn emit<T: Serialize>(bridge: &Bridge, event: &str, data: T) {
    let callback = bridge.on_event.lock().ok().and_then(|c| *c);
    let Some(callback) = callback else { return };
    let Ok(json) = serde_json::to_string(&Envelope { event, data }) else {
        return;
    };
    let Ok(text) = CString::new(json) else { return };
    callback(text.into_raw());
}

/// Runs `f` against the player and pushes the new state to the listener: the whole state, or just the position
/// when that is all that moved (the queue, with every title and thumbnail, need not travel four times a second).
fn publish(f: impl FnOnce(&mut Player) -> Vec<Effect>) {
    let Some(bridge) = BRIDGE.get() else { return };
    let Ok(mut player) = bridge.player.lock() else {
        return;
    };
    // The effects (play this, prefetch that) are for the audio engine, which is not attached to the bridge yet.
    let _ = f(&mut player);
    let mut dto = state_dto(&player);
    dto.seq = bridge.seq.fetch_add(1, Ordering::SeqCst) + 1;
    let mut key = dto.clone();
    key.seq = 0;
    key.position_ms = 0;
    let only_position = match bridge.last_published.lock() {
        Ok(mut last) => {
            let same = last.as_ref() == Some(&key);
            *last = Some(key);
            same
        }
        Err(_) => false,
    };
    drop(player);
    if only_position {
        let position = PositionDto {
            seq: dto.seq,
            position_ms: dto.position_ms,
        };
        emit(bridge, "position", position);
    } else {
        emit(bridge, "state", dto);
    }
}

#[derive(Deserialize)]
struct Request {
    method: String,
    #[serde(default)]
    params: Value,
}

pub fn call(request: &str) -> String {
    let outcome = catch_unwind(AssertUnwindSafe(|| handle(request)))
        .unwrap_or_else(|_| Err("the bridge panicked".to_string()));
    let reply = match outcome {
        Ok(data) => json!({ "ok": true, "data": data }),
        Err(error) => json!({ "ok": false, "error": error }),
    };
    reply.to_string()
}

pub fn failure(error: &str) -> String {
    json!({ "ok": false, "error": error }).to_string()
}

fn handle(request: &str) -> Result<Value, String> {
    let request: Request =
        serde_json::from_str(request).map_err(|e| format!("bad request: {e}"))?;
    let bridge = BRIDGE.get().ok_or("the bridge is not started")?;
    match request.method.as_str() {
        "getState" => {
            let player = bridge
                .player
                .lock()
                .map_err(|_| "player state unavailable")?;
            let mut dto = state_dto(&player);
            dto.seq = bridge.seq.load(Ordering::SeqCst);
            serde_json::to_value(dto).map_err(|e| e.to_string())
        }
        "dispatch" => {
            let command: CommandDto = serde_json::from_value(request.params)
                .map_err(|e| format!("bad command: {e}"))?;
            publish(|player| player.command(command.into()));
            Ok(Value::Null)
        }
        other => Err(format!("unknown method: {other}")),
    }
}
