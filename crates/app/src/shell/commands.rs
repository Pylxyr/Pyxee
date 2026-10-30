//! Commands the UI calls for the shell features.

use super::dto::{LyricsDto, ShellSettingsDto, SleepSettingDto, SleepStateDto, SleepStatusDto};
use super::{flyout, mini, sleep, ShellState};
use crate::state::now_ms;
use pyxee_lyrics::{fetch, Query, LRCLIB};
use std::io::Read;
use std::time::Duration;
use tauri::{AppHandle, Manager};

const LYRICS_CACHE_MAX: usize = 64;
/// The most a cover may weigh for the app to fetch it (a 64 px thumbnail is a few KB).
const COVER_MAX_BYTES: u64 = 512 * 1024;

/// True for an https address on YouTube's or Google's image hosts, the only places covers come from.
fn cover_host_ok(url: &str) -> bool {
    pyxee_sources::is_cover_host(url)
}

/// The bytes of a cover, for the colour of the album art. The page cannot read the pixels of an image from another
/// site unless that site allows it, so the app fetches the (small) picture itself and hands it over.
#[tauri::command]
pub async fn cover_bytes(url: String) -> Result<tauri::ipc::Response, String> {
    if !cover_host_ok(&url) {
        return Err("not a cover address".into());
    }
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(8))
            .redirects(0)
            .build();
        let response = agent.get(&url).call().map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(COVER_MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > COVER_MAX_BYTES {
            return Err("the cover is too large".to_string());
        }
        Ok(bytes)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(bytes))
}

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
        lastfm_scrobbling: false,
        borderless: true,
    })
}

#[tauri::command]
pub fn shell_set(app: AppHandle, settings: ShellSettingsDto) -> Result<ShellSettingsDto, String> {
    let shell = app.state::<ShellState>();
    let lastfm_turned_on;
    {
        let mut s = shell
            .settings
            .lock()
            .map_err(|_| "Settings are unavailable.".to_string())?;
        lastfm_turned_on = !s.lastfm_scrobbling && settings.lastfm_scrobbling;
        s.close_to_tray = settings.close_to_tray;
        s.start_with_windows = settings.start_with_windows;
        s.toast_on_change = settings.toast_on_change;
        s.tray_controls = settings.tray_controls;
        s.flyout_on_click = settings.flyout_on_click;
        s.discord_presence = settings.discord_presence;
        s.lastfm_scrobbling = settings.lastfm_scrobbling;
        s.borderless = settings.borderless;
    }
    shell.save_settings();
    super::sync_autostart(&app);
    super::tray_buttons::sync(&app, settings.tray_controls);
    super::sync_discord(&app);
    super::sync_lastfm(&app, lastfm_turned_on);
    super::sync_window_frame(&app);
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
