//! The C interface the Flutter shell (`ui_flutter/`) loads. Requests go in and replies come back as JSON text, and
//! events are pushed to a callback, so the surface stays small and nothing has to be generated.
//!
//! The Tauri-free modules of `pyxee-app` are included from there with `#[path]` rather than copied, so the old shell
//! and this one run the same engine, library and contract. Only the glue that needs an `AppHandle` is rewritten here
//! (`channel.rs`).

use std::ffi::{c_char, CStr, CString};
use std::ptr;

mod channel;

/// The typed contract between the core and the UI: the Tauri shell's own file.
#[path = "../../app/src/dto.rs"]
pub mod dto;
/// The shell features' contract (lyrics, sleep timer), also the Tauri shell's file.
#[path = "../../app/src/shell/dto.rs"]
pub mod shell_dto;
#[path = "../../app/src/engine.rs"]
pub mod engine;
#[path = "../../app/src/library_service.rs"]
pub mod library_service;
#[path = "../../app/src/recorder.rs"]
pub mod recorder;
#[path = "../../app/src/session.rs"]
pub mod session;
#[path = "../../app/src/tools.rs"]
pub mod tools;

/// Changes whenever an exported function changes incompatibly, so the shell can refuse a library that does not match.
pub const ABI_VERSION: u32 = 3;

#[no_mangle]
pub extern "C" fn pyxee_bridge_abi_version() -> u32 {
    ABI_VERSION
}

/// Starts the core (once per process; calling it again only replaces the listener) and sends the first state to
/// `on_event`, which may be called from any thread with a JSON string to give back to `pyxee_bridge_free`.
/// Returns 1 when started, 0 when `on_event` is null.
#[no_mangle]
pub extern "C" fn pyxee_bridge_start(on_event: Option<channel::EventCallback>) -> u32 {
    match on_event {
        Some(callback) => {
            channel::start(callback);
            1
        }
        None => 0,
    }
}

/// Stops events from being sent; call it before the listener is closed.
#[no_mangle]
pub extern "C" fn pyxee_bridge_stop() {
    channel::stop();
}

/// Answers one JSON request (`{"method": ..., "params": ...}`) with one JSON reply (`{"ok": true, "data": ...}` or
/// `{"ok": false, "error": ..., "kind": ...}`), on the calling thread. Use it for requests that return at once.
/// Returns null only when the reply cannot be made into a C string.
///
/// # Safety
/// `request` must be null or point to a NUL-terminated string. The reply must be given back to `pyxee_bridge_free`.
#[no_mangle]
pub unsafe extern "C" fn pyxee_bridge_call(request: *const c_char) -> *mut c_char {
    let reply = if request.is_null() {
        channel::failure("no request")
    } else {
        match CStr::from_ptr(request).to_str() {
            Ok(text) => channel::call(text),
            Err(_) => channel::failure("the request is not UTF-8"),
        }
    };
    CString::new(reply).map_or(ptr::null_mut(), CString::into_raw)
}

/// Like `pyxee_bridge_call`, but returns at once: the request runs on its own thread, and the reply arrives as a
/// `reply` event `{"id": id, "reply": {...}}`. Use it for anything that can take a moment (search, network).
///
/// # Safety
/// `request` must be null or point to a NUL-terminated string; it is copied before this returns.
#[no_mangle]
pub unsafe extern "C" fn pyxee_bridge_call_async(request: *const c_char, id: u64) {
    let text = if request.is_null() {
        None
    } else {
        CStr::from_ptr(request).to_str().ok().map(str::to_owned)
    };
    channel::call_async(text, id);
}

/// Gives back a string that `pyxee_bridge_call` returned or that was passed to the event callback.
///
/// # Safety
/// `text` must be null or such a string, and must not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn pyxee_bridge_free(text: *mut c_char) {
    if !text.is_null() {
        drop(CString::from_raw(text));
    }
}
