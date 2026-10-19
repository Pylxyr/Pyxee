//! Commands the UI calls for the shell features.

use super::dto::{LyricsDto, ShellSettingsDto, SleepSettingDto, SleepStateDto, SleepStatusDto};
use super::{flyout, mini, sleep, ShellState};
use crate::state::now_ms;
use pyxee_lyrics::{fetch, Query, LRCLIB};
use tauri::{AppHandle, Manager};

const LYRICS_CACHE_MAX: usize = 64;

/// Synced lyrics for a song, from LRCLIB. Answers are kept for the session so flipping tabs never asks twice.
#[tauri::command]
pub async fn lyrics_get(
    app: AppHandle,
    key: String,
    title: String,
    artist: String,
    duration_ms: Option<u64>,
) -> LyricsDto {
    let shell = app.state::<ShellState>();
    if let Some(hit) = shell.lyrics.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit.into();
    }
    let now_ms = || u64::try_from(shell.clock.elapsed().as_millis()).unwrap_or(u64::MAX);
    // A service that just refused the app is left alone, so flipping songs cannot hammer a block.
    let waiting = shell
        .lyrics_cooldown
        .lock()
        .ok()
        .and_then(|c| c.remaining_secs(now_ms()));
    if let Some(secs) = waiting {
        return LyricsDto::Error {
            message: format!(
                "the lyrics service asked the app to wait, try again in about {secs} s"
            ),
        };
    }
    let query = Query::new(&title, &artist, duration_ms);
    let found = tauri::async_runtime::spawn_blocking(move || fetch(LRCLIB, &query)).await;
    match found {
        Ok(Ok(lyrics)) => {
            if let Ok(mut cache) = shell.lyrics.lock() {
                if cache.len() >= LYRICS_CACHE_MAX {
                    cache.clear();
                }
                cache.insert(key, lyrics.clone());
            }
            lyrics.into()
        }
        Ok(Err(e)) => {
            log::info!("lyrics lookup failed: {e}");
            if let Ok(mut c) = shell.lyrics_cooldown.lock() {
                c.record(&e, now_ms());
            }
            LyricsDto::Error {
                message: e.to_string(),
            }
        }
        Err(e) => LyricsDto::Error {
            message: format!("lyrics lookup stopped: {e}"),
        },
    }
}

#[tauri::command]
pub fn sleep_set(app: AppHandle, setting: SleepSettingDto) {
    sleep::set(&app, setting.into());
}

#[tauri::command]
pub fn sleep_cancel(app: AppHandle) {
    sleep::cancel(&app);
}

#[tauri::command]
pub fn sleep_extend(app: AppHandle, minutes: u32) {
    sleep::extend(&app, minutes);
}

#[tauri::command]
pub fn sleep_get(app: AppHandle) -> SleepStateDto {
    let shell = app.state::<ShellState>();
    let status = shell
        .sleep
        .lock()
        .ok()
        .and_then(|t| t.status(now_ms().max(0) as u64))
        .map(SleepStatusDto::from);
    SleepStateDto { status }
}

#[tauri::command]
pub fn shell_get(app: AppHandle) -> ShellSettingsDto {
    let shell = app.state::<ShellState>();
    let settings = shell.settings.lock().map(|s| ShellSettingsDto::from(&*s));
    settings.unwrap_or(ShellSettingsDto {
        close_to_tray: false,
        start_with_windows: false,
        toast_on_change: false,
        tray_controls: false,
        flyout_on_click: true,
        discord_presence: true,
    })
}

#[tauri::command]
pub fn shell_set(app: AppHandle, settings: ShellSettingsDto) -> Result<ShellSettingsDto, String> {
    let shell = app.state::<ShellState>();
    {
        let mut s = shell
            .settings
            .lock()
            .map_err(|_| "Settings are unavailable.".to_string())?;
        s.close_to_tray = settings.close_to_tray;
        s.start_with_windows = settings.start_with_windows;
        s.toast_on_change = settings.toast_on_change;
        s.tray_controls = settings.tray_controls;
        s.flyout_on_click = settings.flyout_on_click;
        s.discord_presence = settings.discord_presence;
    }
    shell.save_settings();
    super::sync_autostart(&app);
    super::tray_buttons::sync(&app, settings.tray_controls);
    super::sync_discord(&app);
    Ok(settings)
}

#[tauri::command]
pub fn flyout_hide(app: AppHandle) {
    flyout::hide(&app);
}

/// "Open Pyxee" from the flyout.
#[tauri::command]
pub fn flyout_open_main(app: AppHandle) {
    flyout::hide(&app);
    super::show_main(&app);
}

#[tauri::command]
pub fn mini_toggle(app: AppHandle) {
    mini::toggle(&app);
}

#[tauri::command]
pub fn main_show(app: AppHandle) {
    super::show_main(&app);
}
