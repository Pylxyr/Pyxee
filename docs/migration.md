# Migration from Tauri + Svelte to Flutter

Pyxee used to be a Tauri shell (`crates/app`) around a Svelte UI (`ui/`). The Flutter app in `ui_flutter/` is now the
only UI, and the Rust core is reached through the C interface in `crates/bridge`. Everything Tauri and Svelte did is
still in git: the tag `tauri-final` points at the tip of `main` before the Flutter work, the last commit that has both trees as they were
(and no Flutter app).
To read or revive a file, for example the updater:

```
git show tauri-final:crates/app/src/updater.rs
```

This page lists what carried over and what did not. The code behind each lost feature is not kept in the tree.

## What exists in Flutter now

| Feature | Where it lives now |
|---|---|
| Playback engine, queue, gapless playback, history | `crates/bridge/src/engine.rs` and `crates/core` (moved from `crates/app/src`) |
| Library, playlists, liked songs, settings storage | `crates/bridge/src/library_service.rs`, `crates/library` |
| Search with suggestions, explore, home, related, lyrics | bridge calls in `crates/bridge/src/channel.rs`, pages in `ui_flutter/lib/features` |
| Sleep timer with volume fade | `crates/bridge/src/channel.rs` (uses `pyxee_shell::sleep`), UI in Now Playing and Settings |
| Media keys and the system media tile (SMTC on Windows, MPRIS on Linux) | `crates/bridge/src/channel.rs` (`souvlaki`) |
| In-app updates: a check at start, the "What's new" sheet, installing Pyxee, updating yt-dlp | `crates/bridge/src/updater.rs`, `ui_flutter/lib/core/state/updates.dart`, `ui_flutter/lib/features/settings/updates_section.dart` and `changelog_sheet.dart` |
| Settings > Advanced settings (the `.env` editor) | `crates/bridge/src/advanced.rs`, `ui_flutter/lib/features/settings/advanced_sheet.dart` |
| Discord Rich Presence (needs a build with `PYXEE_DISCORD_CLIENT_ID`) | `crates/bridge/src/discord.rs`, a toggle in Settings |
| Tray icon, close to tray, window drag and maximise, Mica | `ui_flutter/lib/desktop` (`tray_manager`, `window_manager`, `flutter_acrylic`) |
| 22 built-in themes, match system, colours from album art, theme editor with import and export | `ui_flutter/lib/features/settings`, `ui_flutter/lib/core` |

The Tauri glue that wired these to a window (`main.rs`, `state.rs`, `runner.rs`, `commands.rs`, `window.rs`, `opener.rs`,
`shell/mod.rs`, `shell/commands.rs`, `shell/media.rs`, `shell/sleep.rs`, `shell/tray.rs`) is replaced by `channel.rs`
and the Flutter shell, so it is not carried over. Settings in the `.env` file in the data folder (cookies, yt-dlp path,
JavaScript runtime and so on, see `docs/advanced.md`) are read at start, and **Settings > Advanced settings** edits them.

## Lost with the removal

"Needs" says what it takes to bring the feature back: **bridge method** means a new call in `channel.rs` plus Flutter
UI; **new Rust shell code** means platform code that Tauri used to provide (a window, a plugin or a webview) has to be
written again outside Tauri.

| Feature | Lived in (all under `crates/app/src`) | Needs |
|---|---|---|
| Last.fm scrobbling | `shell/lastfm.rs` | Bridge methods and a settings page |
| Start with Windows | the `tauri-plugin-autostart` setup in `main.rs` and `sync_autostart` in `shell/mod.rs` | New Rust shell code or a Flutter package |
| Single instance (starting Pyxee again focuses the first window) | the `tauri-plugin-single-instance` setup in `main.rs` | New Rust shell code or a Flutter package; no equivalent was found in `ui_flutter` |
| Notifications (for example a skipped track) | the `tauri-plugin-notification` use in `shell/mod.rs` | New Rust shell code or a Flutter package |
| Mini player | `shell/mini.rs` | A second Flutter window; new Rust shell code or a Flutter package |
| Tray flyout | `shell/flyout.rs` | A Flutter popup window; new Rust shell code or a Flutter package |
| Previous / play-pause / next tray icons | `shell/tray_buttons.rs` | New Rust shell code |
| Taskbar thumbnail buttons and the jump list | `shell/winextras.rs` (feature `win-extras`) | New Rust shell code |
| Reacting to the PC waking from sleep and to network changes (a polling thread that sends player commands) | `shell/power.rs` (decisions stay in `pyxee_shell::power`) | New Rust code in the bridge; it needs no window |
| Log file `logs/pyxee.log` (2 MB rotation, repeated lines collapsed, levels `error` to `trace`) and Settings > Diagnostics > Open log folder | `logging.rs` | Bridge: a file logger in `init_logging` and a way to show the folder. Today the bridge writes to stderr only, and the level, set with `PYXEE_LOG`, is `error`, `warn`, `info`, `debug`, `trace` or `off` |
| Performance log and seek-bar experiments (`PYXEE_PERF`, `PYXEE_SEEK_MODE`) | the Svelte UI and `PerfReportDto` | Nothing: they counted WebView work. Profile Flutter with `flutter run --profile` and DevTools |
| Downloads | no Rust file: the Svelte Library page only showed a placeholder tab, and Flutter keeps it | Not implemented in either UI |
| Simulator run mode (`PYXEE_SIM=1`, a fake engine for UI work without audio) | `sim.rs`, the switch in `runner.rs` and `commands.rs` | New Rust shell code. `sim.rs` is kept as a test module only |
| Webview memory trimming and webview sleep | `shell/trim.rs`, `shell/webview_power.rs` | Nothing: they only existed because of WebView2 |

The Rust crates those files used (`pyxee-shell`, `pyxee-lyrics` and the others) are untouched, so the logic and its tests
are still there.

## Rust code deleted with the removal

These types in `crates/bridge/src/dto.rs` were only used by the Tauri shell and by nothing in the bridge: `LogInfoDto`,
`BackdropDto`, `WindowInfoDto` and `PerfReportDto`. The `ts-rs` dependency and every
`#[derive(TS)]` and `#[ts(...)]` attribute are gone too (the Svelte UI read the generated TypeScript; the Flutter app
reads JSON with hand-written Dart models). Both are in the `tauri-final` tag.
