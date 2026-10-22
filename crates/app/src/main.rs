// Hide the console window in release builds on Windows.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod advanced;
mod commands;
mod dto;
mod engine;
#[cfg(test)]
mod engine_tests;
mod library_service;
mod logging;
mod recorder;
mod runner;
mod session;
mod shell;
mod sim;
mod state;
mod tools;
mod updater;
mod window;

use pyxee_core::Player;
use pyxee_sources::local::LocalSource;
use pyxee_sources::ytdlp::{Cookies, YtDlpSource, FORMAT_SELECTOR};
use pyxee_sources::{CompositeSource, MusicSource};
use state::AppState;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use tauri::Manager;

fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64)
}

/// Portable mode: the data folder next to the program, created if needed. `None` when this is an installed copy, or
/// when the folder cannot be created (a read-only location): the normal folders are used then, and the log says why.
fn portable_data_folder() -> Option<pyxee_shell::datadir::Resolved> {
    use pyxee_shell::datadir::{Note, Resolved};
    let dir = updater::portable_dir()?;
    match std::fs::create_dir_all(&dir) {
        Ok(()) => Some(Resolved {
            notes: vec![Note {
                warn: false,
                text: format!("portable mode: everything is kept in {}", dir.display()),
            }],
            dir: Some(dir),
        }),
        Err(e) => {
            eprintln!("portable mode: cannot create {}: {e}", dir.display());
            None
        }
    }
}

/// Prefers the updater's copy of yt-dlp (see `updater`), unless the developer pinned one with `PYXEE_YTDLP`.
fn with_updated_ytdlp(source: YtDlpSource, data_dir: Option<&std::path::Path>) -> YtDlpSource {
    match data_dir {
        Some(dir) if std::env::var_os("PYXEE_YTDLP").is_none_or(|v| v.is_empty()) => {
            source.with_preferred(updater::managed_ytdlp_path(dir))
        }
        _ => source,
    }
}

fn main() {
    tauri::Builder::default()
        // First, so a second launch is caught before anything else starts: it hands its arguments to the
        // running player (jump-list tasks, `--next`) and brings it to the front.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            shell::on_second_launch(app, &argv);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // Where the files live: the Local application-data folder. Files from the old Roaming folder are
            // copied over once, before anything opens them (see `pyxee_shell::datadir`).
            // A portable copy (`portable.txt` next to the program) keeps everything in `data\\` beside it instead.
            let moved = portable_data_folder().unwrap_or_else(|| {
                pyxee_shell::datadir::resolve(
                    app.path().app_data_dir().ok().as_deref(),
                    app.path().app_local_data_dir().ok().as_deref(),
                )
            });
            let data_dir = moved.dir;
            // The `.env` file in the data folder (Settings > Advanced) is applied before anything reads the
            // environment, the log level included.
            let env_file = data_dir.as_ref().map(|d| d.join(".env"));
            let env_applied = match &env_file {
                Some(file) => pyxee_shell::envfile::apply(file),
                None => Default::default(),
            };
            // Logging first, so everything that follows (and every panic) is recorded.
            let log_dir = data_dir
                .as_ref()
                .map_or_else(|| std::env::temp_dir().join("pyxee"), |d| d.join("logs"));
            let (log, log_error) = logging::init(&log_dir);
            if let Some(e) = &log_error {
                eprintln!("{e}");
            }
            for note in &moved.notes {
                if note.warn {
                    log::warn!("data folder: {}", note.text);
                } else {
                    log::info!("data folder: {}", note.text);
                }
            }
            if let Some(warning) = &env_applied.warning {
                log::warn!("settings: {warning}");
            }
            for name in &env_applied.from_windows {
                log::info!("settings: {name} is set in Windows, so .env is not used for it");
            }
            app.manage(advanced::EnvState {
                file: env_file,
                from_windows: env_applied.from_windows,
            });
            let session_path = data_dir.as_ref().map(|d| d.join("session.json"));
            let player = session_path
                .as_deref()
                .and_then(session::load)
                .map_or_else(|| Player::new(seed()), Player::restore);

            let backdrop = app
                .get_webview_window("main")
                .map_or(dto::BackdropDto::None, |w| window::apply_backdrop(&w));

            let (tx, rx) = mpsc::channel();
            log::info!("yt-dlp audio format preference: {FORMAT_SELECTOR}");
            let source: Arc<dyn MusicSource> = Arc::new(CompositeSource {
                remote: with_updated_ytdlp(
                    YtDlpSource::new(tools::locate("yt-dlp", "PYXEE_YTDLP"))
                        .with_cookies(Cookies::from_env()),
                    data_dir.as_deref(),
                ),
                local: LocalSource,
            });
            let library = library_service::LibraryService::open_or_memory(
                data_dir.as_ref().map(|d| d.join("library.db")).as_deref(),
            );
            app.manage(AppState {
                log,
                log_error,
                volume: pyxee_audio::pipeline::Volume::default(),
                library: Mutex::new(library),
                source,
                player: Mutex::new(player),
                engine: Mutex::new(tx),
                backdrop,
                session_path,
            });
            app.manage(shell::ShellState::load(
                data_dir.as_ref().map(|d| d.join("shell.json")),
            ));
            runner::spawn(app.handle().clone(), rx);
            shell::init(app.handle(), &std::env::args().collect::<Vec<_>>());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                window.app_handle().state::<AppState>().save_session();
            }
            shell::on_window_event(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            commands::dispatch,
            commands::get_state,
            commands::window_info,
            commands::dev_catalog,
            commands::search,
            commands::suggest,
            commands::related_get,
            commands::explore_get,
            updater::update_check_app,
            updater::update_install_app,
            updater::update_check_ytdlp,
            updater::update_install_ytdlp,
            commands::search_filters,
            commands::get_library,
            commands::library_set_liked,
            commands::liked_get,
            commands::playlist_create,
            commands::playlist_rename,
            commands::playlist_delete,
            commands::playlist_restore,
            commands::playlist_add,
            commands::playlist_get,
            commands::playlist_remove_entry,
            commands::playlist_restore_entry,
            commands::playlist_move_entry,
            commands::history_get,
            commands::history_clear,
            commands::history_restore,
            commands::home_get,
            commands::set_volume,
            commands::get_log_info,
            commands::open_log_folder,
            advanced::env_get,
            advanced::env_set,
            commands::set_window_theme,
            shell::commands::lyrics_get,
            shell::commands::sleep_set,
            shell::commands::sleep_cancel,
            shell::commands::sleep_extend,
            shell::commands::sleep_get,
            shell::commands::shell_get,
            shell::commands::shell_set,
            shell::commands::flyout_hide,
            shell::commands::flyout_open_main,
            shell::commands::mini_toggle,
            shell::commands::main_show
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pyxee");
}
