//! Windows shell integration (tray, flyout, mini player, media keys, taskbar, sleep timer, power events).
//! The decisions live in `pyxee-shell` / `pyxee-lyrics` and are unit tested there; this module only connects them to
//! Tauri. Anything that needs real Windows, a tray or a display can only be checked on the user's PC.

pub mod commands;
pub mod dto;
mod flyout;
mod media;
mod mini;
mod power;
mod sleep;
mod tray;
mod tray_buttons;
#[cfg(all(windows, feature = "win-extras"))]
mod winextras;

use crate::dto::{PlayerStateDto, TransportDto};
use crate::state::AppState;
use pyxee_core::{Command, EngineEvent};
use pyxee_lyrics::{Cooldown, Lyrics};
use pyxee_shell::cli::{parse_args, CliAction};
use pyxee_shell::progress::{taskbar_progress, Phase, TaskbarProgress};
use pyxee_shell::settings::{self, ShellSettings};
use pyxee_shell::sleep::SleepTimer;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt as _;

pub const MAIN: &str = "main";

/// What was last pushed to the OS, so unchanged values are not sent again on every position tick.
#[derive(Default)]
struct Sent {
    progress: Option<TaskbarProgressKey>,
    track: Option<(u64, Option<String>)>,
    playback: Option<(u8, u64)>,
    /// The transport phase the notification-area buttons last showed.
    controls: Option<Phase>,
    #[cfg(all(windows, feature = "win-extras"))]
    thumbs: Option<(Phase, bool)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TaskbarProgressKey(TaskbarProgress);

pub struct ShellState {
    pub settings: Mutex<ShellSettings>,
    pub settings_path: Option<PathBuf>,
    pub sleep: Mutex<SleepTimer>,
    pub lyrics: Mutex<HashMap<String, Lyrics>>,
    /// When the lyrics service refused the app, how long to leave it alone (see `pyxee_lyrics::Cooldown`).
    pub lyrics_cooldown: Mutex<Cooldown>,
    /// The cooldown's clock: milliseconds since the app started.
    pub clock: Instant,
    /// Set by "Quit" so closing the window really ends the app even with close-to-tray on.
    pub quitting: AtomicBool,
    pub flyout_hidden_at: Mutex<Option<Instant>>,
    /// After a sleep timer fired its fade-out stays at silence until playback starts again.
    restore_fade: AtomicBool,
    /// The song that is audible now (for "a song ended" and for the change toast).
    audible: Mutex<Option<u64>>,
    sent: Mutex<Sent>,
    media: Mutex<Option<media::Media>>,
    mini_dirty: AtomicBool,
    mini_saved: Mutex<Option<Instant>>,
}

impl ShellState {
    pub fn load(path: Option<PathBuf>) -> Self {
        let (settings, warning) = match path.as_deref() {
            Some(p) => {
                let l = settings::load(p);
                (l.settings, l.warning)
            }
            None => (ShellSettings::default(), None),
        };
        if let Some(w) = &warning {
            log::warn!("{w}");
        }
        Self {
            settings: Mutex::new(settings),
            settings_path: path,
            sleep: Mutex::new(SleepTimer::new()),
            lyrics: Mutex::new(HashMap::new()),
            lyrics_cooldown: Mutex::new(Cooldown::default()),
            clock: Instant::now(),
            quitting: AtomicBool::new(false),
            flyout_hidden_at: Mutex::new(None),
            restore_fade: AtomicBool::new(false),
            audible: Mutex::new(None),
            sent: Mutex::new(Sent::default()),
            media: Mutex::new(None),
            mini_dirty: AtomicBool::new(false),
            mini_saved: Mutex::new(None),
        }
    }

    pub fn save_settings(&self) {
        let (Some(path), Ok(s)) = (&self.settings_path, self.settings.lock()) else {
            return;
        };
        if let Err(e) = settings::save(path, &s) {
            log::warn!("could not save shell settings: {e}");
        }
    }
}

/// Called once from `setup`, after `AppState` is managed.
pub fn init(app: &AppHandle, args: &[String]) {
    let cli = parse_args(args);
    let hide_to_tray = cli.autostart
        && app
            .state::<ShellState>()
            .settings
            .lock()
            .is_ok_and(|s| s.close_to_tray);
    if let Err(e) = tray::init(app) {
        log::warn!("could not create the tray icon: {e}");
    }
    let controls = app
        .state::<ShellState>()
        .settings
        .lock()
        .is_ok_and(|s| s.tray_controls);
    tray_buttons::sync(app, controls);
    sleep::spawn(app.clone());
    power::spawn(app.clone());
    if let Ok(mut m) = app.state::<ShellState>().media.lock() {
        *m = media::init(app);
    }
    sync_autostart(app);
    #[cfg(all(windows, feature = "win-extras"))]
    winextras::init(app);
    if let Some(w) = app.get_webview_window(MAIN) {
        if hide_to_tray {
            log::info!("started by Windows at sign-in: staying in the tray");
        } else {
            let _ = w.show();
        }
    }
    run_actions(app, &cli.actions);
}

/// Makes Windows' start-with-sign-in match the saved setting (the user may have removed it in Task Manager).
pub fn sync_autostart(app: &AppHandle) {
    let want = app
        .state::<ShellState>()
        .settings
        .lock()
        .is_ok_and(|s| s.start_with_windows);
    let manager = app.autolaunch();
    match manager.is_enabled() {
        Ok(have) if have != want => {
            let r = if want {
                manager.enable()
            } else {
                manager.disable()
            };
            if let Err(e) = r {
                log::warn!(
                    "could not {} start with Windows: {e}",
                    if want { "enable" } else { "disable" }
                );
            }
        }
        Ok(_) => {}
        Err(e) => log::warn!("could not read the start-with-Windows state: {e}"),
    }
}

/// A second launch or a jump-list task: forward its actions to the running player.
pub fn on_second_launch(app: &AppHandle, argv: &[String]) {
    let cli = parse_args(argv);
    if cli.actions.is_empty() {
        show_main(app);
    }
    run_actions(app, &cli.actions);
}

pub(crate) fn run_actions(app: &AppHandle, actions: &[CliAction]) {
    for a in actions {
        match a {
            CliAction::PlayPause => command(app, Command::Toggle),
            CliAction::Play => command(app, Command::Resume),
            CliAction::Pause => command(app, Command::Pause),
            CliAction::Next => command(app, Command::Next),
            CliAction::Previous => command(app, Command::Previous),
            CliAction::Mini => mini::toggle(app),
        }
    }
}

pub fn command(app: &AppHandle, c: Command) {
    crate::state::publish(app, |p| p.command(c));
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Really ends the app: saves, flushes settings, and exits even if close-to-tray is on.
pub fn quit(app: &AppHandle) {
    let shell = app.state::<ShellState>();
    shell.quitting.store(true, Ordering::Relaxed);
    mini::flush(app);
    app.state::<AppState>().save_session();
    app.exit(0);
}

/// Window events from `main.rs`.
pub fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    let app = window.app_handle();
    match (window.label(), event) {
        (MAIN, tauri::WindowEvent::CloseRequested { api, .. }) => {
            let shell = app.state::<ShellState>();
            let to_tray = shell.settings.lock().is_ok_and(|s| s.close_to_tray);
            if to_tray && !shell.quitting.load(Ordering::Relaxed) {
                api.prevent_close();
                let _ = window.hide();
            } else {
                quit(app);
            }
        }
        (flyout::LABEL, tauri::WindowEvent::Focused(false)) => flyout::hide(app),
        (mini::LABEL, tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)) => {
            mini::on_moved(app)
        }
        (mini::LABEL, tauri::WindowEvent::CloseRequested { api, .. }) => {
            // Closing the mini player only hides it; it keeps its place.
            api.prevent_close();
            mini::hide(app);
        }
        _ => {}
    }
}

/// After every state change the player publishes. Cheap: it only touches the OS when something visible changed.
pub fn on_state(app: &AppHandle, s: &PlayerStateDto) {
    let Some(shell) = app.try_state::<ShellState>() else {
        return;
    };
    let entry = s.current.and_then(|id| s.queue.iter().find(|e| e.id == id));
    let duration = entry.and_then(|e| e.track.duration_ms);
    let (phase, id) = match &s.transport {
        TransportDto::Idle => (Phase::Idle, None),
        TransportDto::Loading { id } | TransportDto::Stalled { id } => (Phase::Loading, Some(*id)),
        TransportDto::Playing { id } => (Phase::Playing, Some(*id)),
        TransportDto::Paused { id, .. } => (Phase::Paused, Some(*id)),
        TransportDto::Failed { id, .. } => (Phase::Failed, Some(*id)),
    };

    if phase == Phase::Playing && shell.restore_fade.swap(false, Ordering::Relaxed) {
        app.state::<AppState>().volume.set_fade(1.0);
    }

    let Ok(mut sent) = shell.sent.lock() else {
        return;
    };

    let progress = taskbar_progress(phase, s.position_ms, duration);
    if sent.progress != Some(TaskbarProgressKey(progress)) {
        sent.progress = Some(TaskbarProgressKey(progress));
        apply_taskbar_progress(app, progress);
    }

    #[cfg(all(windows, feature = "win-extras"))]
    {
        let key = (phase, entry.is_some());
        if sent.thumbs != Some(key) {
            sent.thumbs = Some(key);
            winextras::update(app, phase, entry.is_some());
        }
    }

    let track_key = entry.map(|e| (e.id, e.track.thumbnail.clone()));
    if sent.track != track_key {
        sent.track.clone_from(&track_key);
        if let Some(e) = entry {
            tray::set_tooltip(
                app,
                &pyxee_shell::tray_controls::track_tooltip(&e.track.title, &e.track.artist),
            );
        } else {
            tray::set_tooltip(app, "Pyxee");
        }
        if let (Some(m), Ok(mut guard)) = (entry, shell.media.lock()) {
            if let Some(media) = guard.as_mut() {
                media.set_track(&m.track);
            }
        }
        sent.playback = None; // new song: send its state afresh
    }

    if sent.controls != Some(phase) {
        sent.controls = Some(phase);
        tray_buttons::update(app, phase);
    }

    let key = (
        match phase {
            Phase::Playing => 1,
            Phase::Paused => 2,
            Phase::Idle => 0,
            _ => 3,
        },
        s.position_ms / 1000,
    );
    if sent.playback != Some(key) {
        sent.playback = Some(key);
        if let Ok(mut guard) = shell.media.lock() {
            if let Some(media) = guard.as_mut() {
                media.set_playback(phase, s.position_ms, id.is_some());
            }
        }
    }
}

fn apply_taskbar_progress(app: &AppHandle, p: TaskbarProgress) {
    use tauri::window::{ProgressBarState, ProgressBarStatus};
    let Some(w) = app.get_webview_window(MAIN) else {
        return;
    };
    let (status, progress) = match p {
        TaskbarProgress::None => (ProgressBarStatus::None, None),
        TaskbarProgress::Indeterminate => (ProgressBarStatus::Indeterminate, None),
        TaskbarProgress::Normal(n) => (ProgressBarStatus::Normal, Some(u64::from(n))),
        TaskbarProgress::Paused(n) => (ProgressBarStatus::Paused, Some(u64::from(n))),
        TaskbarProgress::Error => (ProgressBarStatus::Error, Some(100)),
    };
    if let Err(e) = w.set_progress_bar(ProgressBarState {
        status: Some(status),
        progress,
    }) {
        log::debug!("taskbar progress not applied: {e}");
    }
}

/// Engine events seen after the player has processed them: a song ended (sleep timer), a song became audible (toast).
pub fn on_engine_event(app: &AppHandle, e: &EngineEvent) {
    let Some(shell) = app.try_state::<ShellState>() else {
        return;
    };
    match e {
        EngineEvent::Audible { id } => {
            let changed = shell
                .audible
                .lock()
                .is_ok_and(|mut a| a.replace(id.0) != Some(id.0));
            if changed {
                toast_for(app, id.0);
            }
        }
        EngineEvent::Ended { id } => {
            let genuine = shell
                .audible
                .lock()
                .is_ok_and(|mut a| a.take() == Some(id.0));
            if genuine {
                sleep::on_track_finished(app);
            }
        }
        _ => {}
    }
}

fn toast_for(app: &AppHandle, id: u64) {
    use tauri_plugin_notification::NotificationExt;
    let shell = app.state::<ShellState>();
    if !shell.settings.lock().is_ok_and(|s| s.toast_on_change) {
        return;
    }
    // Only when the window is not in front of the user (the player bar already shows the song then).
    let focused = app
        .get_webview_window(MAIN)
        .is_some_and(|w| w.is_focused().unwrap_or(false));
    if focused {
        return;
    }
    let track = app.state::<AppState>().player.lock().ok().and_then(|p| {
        p.queue()
            .get(pyxee_core::EntryId(id))
            .map(|e| e.track.clone())
    });
    if let Some(t) = track {
        if let Err(e) = app
            .notification()
            .builder()
            .title(&t.title)
            .body(&t.artist)
            .show()
        {
            log::debug!("toast not shown: {e}");
        }
    }
}

/// The player's phase right now (for code that is not handed a state, like the taskbar button callback).
pub(crate) fn current_phase(app: &AppHandle) -> (Phase, bool) {
    let Some(state) = app.try_state::<AppState>() else {
        return (Phase::Idle, false);
    };
    let Ok(p) = state.player.lock() else {
        return (Phase::Idle, false);
    };
    let dto = crate::dto::state_dto(&p);
    let has_track = dto.current.is_some();
    let phase = match dto.transport {
        TransportDto::Idle => Phase::Idle,
        TransportDto::Loading { .. } | TransportDto::Stalled { .. } => Phase::Loading,
        TransportDto::Playing { .. } => Phase::Playing,
        TransportDto::Paused { .. } => Phase::Paused,
        TransportDto::Failed { .. } => Phase::Failed,
    };
    (phase, has_track)
}
