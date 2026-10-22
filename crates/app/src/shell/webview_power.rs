//! Puts the page of a hidden window to sleep.
//!
//! Hiding a window (close to tray, the tray flyout, the mini player) hides only the window. WebView2 is told nothing:
//! the hide and minimize messages go to the window, not to the browser inside it. So it keeps painting frames, running
//! animation callbacks and holding everything an open page holds, for a page nobody can see. That is the steady CPU and
//! GPU use, and the memory, that Task Manager shows under "WebView2 Manager" while Pyxee sits in the tray.
//!
//! [`set_active`]`(window, false)` tells it, in three steps:
//!
//! * `SetIsVisible(false)`: the page counts as hidden, so frames stop and timers are throttled;
//! * `SetMemoryUsageTargetLevel(Low)`: it trims what it holds on to;
//! * `TrySuspend`: it stops running the page altogether.
//!
//! `set_active(window, true)` undoes all three. Music is not affected: it is played by Rust, not by the page. State
//! pushed to a sleeping page may be missed, so the UI asks for it again when the page wakes (`visibilitychange`, see
//! `ui/src/lib/player.svelte.ts`).
//!
//! Only WebView2 (Windows) needs this; elsewhere `set_active` only keeps its bookkeeping.

use std::sync::{Mutex, PoisonError};
use tauri::WebviewWindow;

/// Labels of the windows whose page was put to sleep.
static ASLEEP: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Records what is wanted for `label`; returns whether that differs from what was last asked (so a repeat is skipped).
fn note(label: &str, active: bool) -> bool {
    let mut asleep = ASLEEP.lock().unwrap_or_else(PoisonError::into_inner);
    let is_asleep = asleep.iter().any(|l| l == label);
    if active != is_asleep {
        return false; // already as wanted
    }
    if active {
        asleep.retain(|l| l != label);
    } else {
        asleep.push(label.to_string());
    }
    true
}

/// Wakes (`true`) or sleeps (`false`) the page of `window`. Safe to call as often as you like.
pub fn set_active(window: &WebviewWindow, active: bool) {
    if !note(window.label(), active) {
        return;
    }
    if !imp::apply(window, active) {
        note(window.label(), !active); // not done: do not remember it as done
    }
}

#[cfg(windows)]
mod imp {
    use tauri::WebviewWindow;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Controller, ICoreWebView2_19, ICoreWebView2_3,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use webview2_com::TrySuspendCompletedHandler;
    use windows_core::Interface;

    /// Queues the change on the UI thread (where WebView2 may be called); false if that was not possible.
    pub fn apply(window: &WebviewWindow, active: bool) -> bool {
        let label = window.label().to_string();
        let queued = window.with_webview(move |view| {
            // SAFETY: WebView2 calls, made on the UI thread, which is where `with_webview` runs this.
            unsafe { change(&view.controller(), active, &label) };
        });
        if let Err(e) = &queued {
            log::debug!("could not change the page power state: {e}");
        }
        queued.is_ok()
    }

    /// Every step is tried whatever the others did: an older WebView2 runtime may lack one of them.
    unsafe fn change(controller: &ICoreWebView2Controller, active: bool, label: &str) {
        let core = controller.CoreWebView2().ok();
        if active {
            if let Some(v3) = core.as_ref().and_then(|c| c.cast::<ICoreWebView2_3>().ok()) {
                let _ = v3.Resume();
            }
            if let Err(e) = controller.SetIsVisible(true) {
                log::warn!("could not show the {label} page again: {e}");
            }
            if let Some(v19) = core
                .as_ref()
                .and_then(|c| c.cast::<ICoreWebView2_19>().ok())
            {
                let _ =
                    v19.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL);
            }
            log::debug!("{label} page: awake");
        } else {
            if let Err(e) = controller.SetIsVisible(false) {
                log::warn!("could not mark the {label} page hidden: {e}");
                return; // a visible page cannot be suspended
            }
            if let Some(v19) = core
                .as_ref()
                .and_then(|c| c.cast::<ICoreWebView2_19>().ok())
            {
                let _ = v19.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW);
            }
            if let Some(v3) = core.as_ref().and_then(|c| c.cast::<ICoreWebView2_3>().ok()) {
                let label = label.to_string();
                let done = TrySuspendCompletedHandler::create(Box::new(move |_, _| {
                    log::debug!("{label} page: asleep");
                    Ok(())
                }));
                let _ = v3.TrySuspend(&done);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use tauri::WebviewWindow;

    pub fn apply(_window: &WebviewWindow, _active: bool) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The list is shared by the whole process, so each test uses its own labels.

    #[test]
    fn a_window_starts_awake_and_repeats_are_skipped() {
        assert!(!note("t-awake", true), "already awake: nothing to do");
        assert!(note("t-awake", false), "first sleep");
        assert!(!note("t-awake", false), "asleep already");
        assert!(note("t-awake", true), "first wake");
        assert!(!note("t-awake", true));
    }

    #[test]
    fn windows_are_tracked_separately() {
        assert!(note("t-a", false));
        assert!(note("t-b", false));
        assert!(note("t-a", true));
        assert!(!note("t-a", true));
        assert!(!note("t-b", false), "b is still asleep");
        assert!(note("t-b", true));
    }
}
