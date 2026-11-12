//! The C interface the Flutter shell (`ui_flutter/`) loads. Requests go in and replies come back as JSON text, and
//! events are pushed to a callback, so the surface stays small and nothing has to be generated.
//!
//! The engine, library, session and contract modules (`engine`, `library_service`, `session`, `dto` ...) live in this
//! crate; `channel.rs` is the glue that drives them for the Flutter app.

use std::ffi::{c_char, CStr, CString};
use std::ptr;

mod channel;

/// The typed contract between the core and the UI.
pub mod advanced;
pub mod discord;
pub mod dto;
pub mod engine;
pub mod library_service;
pub mod opener;
pub mod recorder;
pub mod session;
/// The shell features' contract (lyrics, sleep timer).
pub mod shell_dto;
pub mod tools;
pub mod updater;

#[cfg(test)]
mod engine_tests;
#[cfg(test)]
mod sim;

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
