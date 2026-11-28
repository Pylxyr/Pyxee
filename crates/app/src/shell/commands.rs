//! Commands the UI calls for the shell features.

// Tauri hands a command its arguments by value (`State`, `AppHandle`, the deserialised payload): the signature is not ours to change.
#![allow(clippy::needless_pass_by_value)]

use super::dto::{
    AudioDeviceDto, AudioDevicesDto, EqDto, LyricsDto, ShellSettingsDto, SleepSettingDto,
    SleepStateDto, SleepStatusDto,
};
use super::{ShellState, UiPrefsState, flyout, mini, sleep};
use crate::audio_cache::AudioCache;
use crate::state::{AppState, now_ms};
use pyxee_audio::output::{DeviceIdent, list_outputs};
use pyxee_lyrics::{LRCLIB, Lyrics, Query, fetch};
use pyxee_shell::settings::{EqSettings, OUTPUT_DEVICE_ID_MAX};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};

const LYRICS_CACHE_MAX: usize = 64;
/// The most a cover may weigh for the app to fetch it (a 64 px thumbnail is a few KB).
const COVER_MAX_BYTES: u64 = 512 * 1024;

/// True for an https address on `YouTube`'s or Google's image hosts, the only places covers come from.
fn cover_host_ok(url: &str) -> bool {
    pyxee_sources::is_cover_host(url)
}

/// The bytes of a cover, for the colour of the album art. The page cannot read the pixels of an image from another
/// site unless that site allows it, so the app fetches the (small) picture itself and hands it over.
#[tauri::command]
pub async fn cover_bytes(url: String) -> Result<tauri::ipc::Response, String> {
    // The cover of a song on this PC: read from the file itself (the address names it, and only a song is opened).
    if let Some(song) = pyxee_sources::local::cover_url_path(&url) {
        let cover =
            tauri::async_runtime::spawn_blocking(move || pyxee_sources::local::read_cover(&song))
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "this song has no cover".to_string())?;
        return Ok(tauri::ipc::Response::new(cover.bytes));
    }
    if !cover_host_ok(&url) {
        return Err("not a cover address".into());
    }
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        // No redirects: a cover host that sends the app somewhere else is not a cover host this app vetted.
        let agent: ureq::Agent = pyxee_net::agent_config()
            .timeout_global(Some(Duration::from_secs(8)))
            .max_redirects(0)
            .build()
            .into();
        let mut response =
            pyxee_net::retry_stale(|| agent.get(&url).call()).map_err(|e| e.to_string())?;
        // With redirects off a redirect comes back as an ordinary answer: it is not a picture.
        if !response.status().is_success() {
            return Err(format!("the cover address answered {}", response.status()));
        }
        response
            .body_mut()
            .with_config()
            .limit(COVER_MAX_BYTES)
            .read_to_vec()
            .map_err(|e| match e {
                ureq::Error::BodyExceedsLimit(_) => "the cover is too large".to_string(),
                e => e.to_string(),
            })
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Remembers an answer for the session, so flipping tabs never asks twice.
fn remember_lyrics(shell: &ShellState, key: String, lyrics: &Lyrics) {
    if let Ok(mut cache) = shell.lyrics.lock() {
        if cache.len() >= LYRICS_CACHE_MAX {
            cache.clear();
        }
        cache.insert(key, lyrics.clone());
    }
}

/// Synced lyrics for a song, from LRCLIB. Answers are kept for the session so flipping tabs never asks twice, and
/// on disk too while songs are being kept offline (Settings -> Offline), so they show again without the network.
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
    let kept = Arc::clone(&app.state::<Arc<AudioCache>>());
    if kept.enabled() {
        let (store, wanted) = (Arc::clone(&kept), key.clone());
        let on_disk =
            tauri::async_runtime::spawn_blocking(move || store.lyrics_lookup(&wanted)).await;
        if let Ok(Some(hit)) = on_disk {
            remember_lyrics(&shell, key, &hit);
            return hit.into();
        }
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
            remember_lyrics(&shell, key.clone(), &lyrics);
            if kept.enabled() {
                let (store, lyrics) = (Arc::clone(&kept), lyrics.clone());
                if let Err(e) =
                    tauri::async_runtime::spawn_blocking(move || store.lyrics_keep(&key, &lyrics))
                        .await
                {
                    log::warn!("could not keep the lyrics: {e}");
                }
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
        discord_presence: false,
        lastfm_scrobbling: false,
        check_prereleases: false,
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
        s.check_prereleases = settings.check_prereleases;
    }
    shell.save_settings();
    super::sync_autostart(&app);
    super::tray_buttons::sync(&app, settings.tray_controls);
    super::sync_discord(&app);
    super::sync_lastfm(&app, lastfm_turned_on);
    Ok(settings)
}

/// The devices as the window shows them, with the choice that is saved.
fn devices_dto(app: &AppHandle, found: Vec<(DeviceIdent, bool)>) -> AudioDevicesDto {
    let selected = app.try_state::<ShellState>().and_then(|shell| {
        shell
            .settings
            .lock()
            .ok()
            .and_then(|s| s.output_device.clone())
    });
    AudioDevicesDto {
        devices: found
            .into_iter()
            .map(|(d, is_default)| AudioDeviceDto {
                id: d.id,
                name: d.name,
                is_default,
            })
            .collect(),
        selected,
    }
}

/// Settings > Playback > Output device: the devices that are connected and the one chosen. Listing them asks the
/// system, so it runs off the window's thread.
// Tauri hands a command its `AppHandle` by value: the signature is not ours to change.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn audio_devices_get(app: AppHandle) -> Result<AudioDevicesDto, String> {
    let found = tauri::async_runtime::spawn_blocking(list_outputs)
        .await
        .map_err(|e| e.to_string())?;
    Ok(devices_dto(&app, found))
}

/// Chooses the output device (`None`: follow the system default again). Only the id of a device that is connected is
/// accepted, so nothing but what the system itself listed is ever saved. The audio thread follows the change within a
/// moment. Resolves to the list as it is now.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn audio_device_set(
    app: AppHandle,
    id: Option<String>,
) -> Result<AudioDevicesDto, String> {
    if let Some(id) = &id
        && (id.is_empty() || id.len() > OUTPUT_DEVICE_ID_MAX)
    {
        return Err("That is not an output device.".into());
    }
    let found = tauri::async_runtime::spawn_blocking(list_outputs)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(id) = &id
        && !found.iter().any(|(d, _)| d.id == *id)
    {
        return Err("That output device is not connected.".into());
    }
    {
        let shell = app.state::<ShellState>();
        let mut s = shell
            .settings
            .lock()
            .map_err(|_| "Settings are unavailable.".to_string())?;
        s.output_device = id;
    }
    app.state::<ShellState>().save_settings();
    Ok(devices_dto(&app, found))
}

/// Settings > Connections > "Clear Last.fm data": signs out, deletes the saved sign-in and the scrobbles waiting to be
/// sent, and turns scrobbling off. Resolves to the settings now in effect.
#[tauri::command]
pub fn lastfm_clear(app: AppHandle) -> ShellSettingsDto {
    super::clear_lastfm(&app);
    shell_get(app)
}

/// The page's saved settings (see [`UiPrefsState`]): asked once at start, before the page reads its own store.
#[tauri::command]
pub fn ui_prefs_get(app: AppHandle) -> BTreeMap<String, String> {
    app.state::<UiPrefsState>().items()
}

/// Saves the page's settings: the whole set, whenever one of them changed.
#[tauri::command]
pub fn ui_prefs_set(app: AppHandle, items: BTreeMap<String, String>) -> Result<(), String> {
    app.state::<UiPrefsState>()
        .replace(items)
        .map_err(|e| format!("Could not save the settings: {e}"))
}

#[tauri::command]
pub fn eq_get(app: AppHandle) -> EqDto {
    let shell = app.state::<ShellState>();
    let settings = shell.settings.lock().map(|s| EqDto::from(&s.eq));
    settings.unwrap_or_else(|_| EqDto::from(&EqSettings::default()))
}

/// Sets the equalizer: heard from the next audio buffer on. `save` is false while a slider is being dragged (every
/// step would otherwise rewrite the settings file) and true when it is let go or a switch or preset is used.
#[tauri::command]
pub fn eq_set(app: AppHandle, eq: EqDto, save: bool) -> Result<EqDto, String> {
    let eq = EqSettings::from(eq);
    app.state::<AppState>()
        .eq
        .set(eq.enabled, eq.preamp, &eq.gains);
    let shell = app.state::<ShellState>();
    shell
        .settings
        .lock()
        .map_err(|_| "Settings are unavailable.".to_string())?
        .eq = eq.clone();
    if save {
        shell.save_settings();
    }
    Ok(EqDto::from(&eq))
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
