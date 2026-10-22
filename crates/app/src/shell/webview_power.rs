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
//! A sleeping page is also not sent events: [`emit_awake`] skips the windows that are asleep, so the one-second
//! position and sleep-timer pushes do not run script in (and so wake) a page nobody can see. The UI listens per
//! window (`ui/src/lib/backend.ts`) because Tauri delivers to "any"-target listeners whatever the filter says.
//!
//! Whether it worked is logged (debug level): `IsSuspended` right after each `TrySuspend`, and once more about five
//! seconds later. If the page woke up on its own, the log lists what ran script in it meanwhile (see
//! [`note_wake_source`]), so the cause is found rather than guessed.
//!
//! Only WebView2 (Windows) needs this; elsewhere `set_active` only keeps its bookkeeping and `emit_awake` sends to all.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};
use std::time::Instant;
use tauri::{AppHandle, Emitter, EventTarget, WebviewWindow};

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

/// Per window, how many sleep or wake requests it has had. A delayed check made for an older request is stale.
static GENERATION: Mutex<Vec<(String, u64)>> = Mutex::new(Vec::new());

/// Counts a new sleep or wake request for `label` and returns its number.
fn next_generation(label: &str) -> u64 {
    let mut all = GENERATION.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, n)) = all.iter_mut().find(|(l, _)| l == label) {
        *n += 1;
        *n
    } else {
        all.push((label.to_string(), 1));
        1
    }
}

/// Whether `generation` is still the latest request for `label` (nothing slept or woke it since).
#[cfg_attr(not(windows), allow(dead_code))]
fn is_current(label: &str, generation: u64) -> bool {
    GENERATION
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .any(|(l, n)| l == label && *n == generation)
}

/// What ran script in a page lately, newest last: events sent to every window (which skip no one), kept to say what
/// woke a page that did not stay asleep.
static WAKE_LOG: Mutex<VecDeque<(Instant, String)>> = Mutex::new(VecDeque::new());
const WAKE_LOG_LEN: usize = 32;

/// Remembers that `what` was sent to the windows without the asleep filter, so it may have woken a sleeping page.
pub fn note_wake_source(what: String) {
    let mut log = WAKE_LOG.lock().unwrap_or_else(PoisonError::into_inner);
    if log.len() == WAKE_LOG_LEN {
        log.pop_front();
    }
    log.push_back((Instant::now(), what));
}

/// The noted wake sources since `since`, as one line for the log.
#[cfg_attr(not(windows), allow(dead_code))]
fn wake_sources_since(since: Instant) -> String {
    let log = WAKE_LOG.lock().unwrap_or_else(PoisonError::into_inner);
    let found: Vec<String> = log
        .iter()
        .filter(|(at, _)| *at >= since)
        .map(|(at, what)| format!("{what} (+{} ms)", at.duration_since(since).as_millis()))
        .collect();
    if found.is_empty() {
        "nothing the app sent (WebView2 itself, a page request or a window event)".to_string()
    } else {
        found.join(", ")
    }
}

/// Like `app.emit`, to every window, asleep or not: the event is noted as a possible wake source first.
pub fn emit_noted<S: Serialize + Clone>(
    app: &AppHandle,
    event: &str,
    payload: S,
) -> tauri::Result<()> {
    note_wake_source(format!("event {event}"));
    app.emit(event, payload)
}

/// Whether the page of the window labelled `label` is asleep. Always false off Windows, where nothing is put to sleep.
pub fn is_asleep(label: &str) -> bool {
    cfg!(windows)
        && ASLEEP
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|l| l == label)
}

/// Like `app.emit`, but windows whose page is asleep are skipped (they read the state again when they wake).
pub fn emit_awake<S: Serialize + Clone>(
    app: &AppHandle,
    event: &str,
    payload: S,
) -> tauri::Result<()> {
    app.emit_filter(event, payload, |target| match target {
        EventTarget::Window { label }
        | EventTarget::Webview { label }
        | EventTarget::WebviewWindow { label }
        | EventTarget::AnyLabel { label } => !is_asleep(label),
        _ => true,
    })
}

/// Wakes (`true`) or sleeps (`false`) the page of `window`. Safe to call as often as you like.
pub fn set_active(window: &WebviewWindow, active: bool) {
    if !note(window.label(), active) {
        return;
    }
    let generation = next_generation(window.label());
    if !imp::apply(window, active, generation) {
        note(window.label(), !active); // not done: do not remember it as done
    }
}

#[cfg(windows)]
mod imp {
    use std::time::{Duration, Instant};
    use tauri::WebviewWindow;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Controller, ICoreWebView2_19, ICoreWebView2_3,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use webview2_com::TrySuspendCompletedHandler;
    use windows_core::{Interface, BOOL};

    /// How long after a page was put to sleep it is checked once more (is it still suspended, and if not, why).
    const RECHECK_DELAY: Duration = Duration::from_secs(5);

    /// Queues the change on the UI thread (where WebView2 may be called); false if that was not possible.
    /// Putting a page to sleep also queues the one-off check `RECHECK_DELAY` later.
    pub fn apply(window: &WebviewWindow, active: bool, generation: u64) -> bool {
        let label = window.label().to_string();
        let queued = window.with_webview(move |view| {
            // SAFETY: WebView2 calls, made on the UI thread, which is where `with_webview` runs this.
            unsafe { change(&view.controller(), active, &label) };
        });
        if let Err(e) = &queued {
            log::debug!("could not change the page power state: {e}");
        } else if !active {
            recheck_later(window.clone(), generation);
        }
        queued.is_ok()
    }

    /// After `RECHECK_DELAY`, reads `IsSuspended` again, unless the window was woken or put to sleep again meanwhile.
    fn recheck_later(window: WebviewWindow, generation: u64) {
        let since = Instant::now();
        let spawned = std::thread::Builder::new()
            .name("webview-suspend-check".into())
            .spawn(move || {
                std::thread::sleep(RECHECK_DELAY);
                let label = window.label().to_string();
                if !super::is_current(&label, generation) {
                    return; // a newer request owns the page now
                }
                let queued = window.with_webview(move |view| {
                    // SAFETY: WebView2 calls, made on the UI thread, which is where `with_webview` runs this.
                    unsafe { recheck(&view.controller(), &label, since) };
                });
                if let Err(e) = queued {
                    log::debug!("could not check the page power state: {e}");
                }
            });
        if let Err(e) = spawned {
            log::debug!("could not start the page power check: {e}");
        }
    }

    /// `IsSuspended` and `IsVisible` of the page, `None` for a value that could not be read.
    unsafe fn read_state(controller: &ICoreWebView2Controller) -> (Option<bool>, Option<bool>) {
        let suspended = controller
            .CoreWebView2()
            .ok()
            .and_then(|c| c.cast::<ICoreWebView2_3>().ok())
            .and_then(|v3| {
                let mut flag = BOOL::default();
                v3.IsSuspended(&mut flag).ok().map(|()| flag.as_bool())
            });
        let visible = {
            let mut flag = BOOL::default();
            controller
                .IsVisible(&mut flag)
                .ok()
                .map(|()| flag.as_bool())
        };
        (suspended, visible)
    }

    /// The one-off check: still asleep is a debug line, awake again is a warning that names the likely causes.
    unsafe fn recheck(controller: &ICoreWebView2Controller, label: &str, since: Instant) {
        let secs = RECHECK_DELAY.as_secs();
        match read_state(controller) {
            (Some(true), _) => log::debug!("{label} page: still asleep after {secs} s"),
            (Some(false), visible) => log::warn!(
                "{label} page: awake again {secs} s after it was put to sleep (visible: {visible:?}); sent to it meanwhile: {}",
                super::wake_sources_since(since)
            ),
            (None, _) => log::debug!("{label} page: could not read IsSuspended after {secs} s"),
        }
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
                let label_owned = label.to_string();
                let checked = v3.clone();
                let done =
                    TrySuspendCompletedHandler::create(Box::new(move |result, suspended| {
                        let label = &label_owned;
                        // WebView2 may refuse (see its "Sleeping Tabs" conditions); say so instead of claiming it slept.
                        match result {
                            Ok(()) if suspended => log::debug!("{label} page: asleep"),
                            Ok(()) => log::warn!("{label} page: WebView2 declined to suspend it"),
                            Err(e) => log::warn!("{label} page: could not be suspended: {e}"),
                        }
                        // What WebView2 reports now, next to what TrySuspend answered.
                        let mut flag = BOOL::default();
                        // SAFETY: this handler runs on the UI thread, where WebView2 may be called.
                        match unsafe { checked.IsSuspended(&mut flag) } {
                            Ok(()) => log::debug!(
                                "{label} page: IsSuspended = {} after TrySuspend",
                                flag.as_bool()
                            ),
                            Err(e) => log::debug!("{label} page: could not read IsSuspended: {e}"),
                        }
                        Ok(())
                    }));
                if let Err(e) = v3.TrySuspend(&done) {
                    log::warn!("could not ask WebView2 to suspend the {label} page: {e}");
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use tauri::WebviewWindow;

    pub fn apply(_window: &WebviewWindow, _active: bool, _generation: u64) -> bool {
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

    #[cfg(windows)]
    #[test]
    fn is_asleep_follows_the_bookkeeping() {
        assert!(!is_asleep("t-asleep"));
        assert!(note("t-asleep", false));
        assert!(is_asleep("t-asleep"));
        assert!(note("t-asleep", true));
        assert!(!is_asleep("t-asleep"));
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
