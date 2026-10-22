//! Sleep timer glue: a one-second clock, the volume fade, and keeping the player's "stop after current" in step.

use super::dto::{SleepStateDto, SleepStatusDto};
use super::ShellState;
use crate::state::{now_ms, publish, AppState};
use pyxee_core::Command;
use pyxee_shell::sleep::SleepSetting;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub const SLEEP_EVENT: &str = "sleep-state";

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        tick(&app);
    });
}

fn tick(app: &AppHandle) {
    let shell = app.state::<ShellState>();
    let tick = match shell.sleep.lock() {
        Ok(mut t) => t.tick(now_ms().max(0) as u64),
        Err(_) => return,
    };
    let Some(t) = tick else {
        return;
    };
    let volume = &app.state::<AppState>().volume;
    volume.set_fade(t.volume_factor);
    if t.fire {
        log::info!("sleep timer: time is up, pausing");
        publish(app, |p| p.command(Command::Pause));
        // Stay silent until playback is started again, so nothing blips back in before the pause lands.
        shell.restore_fade.store(true, Ordering::Relaxed);
    }
    emit_status(app);
}

pub fn emit_status(app: &AppHandle) {
    let shell = app.state::<ShellState>();
    let status = shell
        .sleep
        .lock()
        .ok()
        .and_then(|t| t.status(now_ms().max(0) as u64))
        .map(SleepStatusDto::from);
    let payload = SleepStateDto { status };
    if let Err(e) = super::emit_awake(app, SLEEP_EVENT, payload) {
        log::warn!("could not send the sleep timer state: {e}");
    }
}

/// The player must know whether the current song is the last one.
pub fn sync_stop_flag(app: &AppHandle) {
    let flag = app
        .state::<ShellState>()
        .sleep
        .lock()
        .is_ok_and(|t| t.stop_after_current());
    publish(app, |p| p.command(Command::StopAfterCurrent(flag)));
}

pub fn set(app: &AppHandle, setting: SleepSetting) {
    let shell = app.state::<ShellState>();
    if let Ok(mut t) = shell.sleep.lock() {
        t.set(setting, now_ms().max(0) as u64);
    }
    app.state::<AppState>().volume.set_fade(1.0);
    sync_stop_flag(app);
    emit_status(app);
}

pub fn cancel(app: &AppHandle) {
    let shell = app.state::<ShellState>();
    if let Ok(mut t) = shell.sleep.lock() {
        t.cancel();
    }
    app.state::<AppState>().volume.set_fade(1.0);
    sync_stop_flag(app);
    emit_status(app);
}

pub fn extend(app: &AppHandle, minutes: u32) {
    let shell = app.state::<ShellState>();
    if let Ok(mut t) = shell.sleep.lock() {
        t.extend_minutes(minutes, now_ms().max(0) as u64);
    }
    // Extending during the fade brings the volume back.
    app.state::<AppState>().volume.set_fade(1.0);
    sync_stop_flag(app);
    emit_status(app);
}

/// A song played to its end (the player has already parked if this was the last one).
pub fn on_track_finished(app: &AppHandle) {
    let shell = app.state::<ShellState>();
    let done = shell.sleep.lock().is_ok_and(|mut t| t.on_track_finished());
    if done {
        log::info!("sleep timer: last song finished, staying paused");
    }
    sync_stop_flag(app);
    emit_status(app);
}
