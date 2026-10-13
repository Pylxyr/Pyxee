# Windows shell integration (milestone 6)

Logic you can test anywhere lives in two crates; `crates/app/src/shell/` only connects it to Tauri and Windows.

| Feature | Decision logic (tested) | Glue |
|---|---|---|
| Synced lyrics | `pyxee-lyrics`: LRC parser (all timestamp forms, offsets, word marks), current-line lookup, title/artist cleanup, LRCLIB client (`/api/get`, then `/api/search`, best match by length and synced-before-plain) | `shell/commands.rs::lyrics_get` (session cache), `ui/.../LyricsPanel.svelte` |
| Sleep timer | `pyxee_shell::sleep`: minutes (fade over the last 15 s), end of song, after N songs; core `Command::StopAfterCurrent` parks on the next song; `Volume::set_fade` is separate from the volume setting | `shell/sleep.rs`, `SleepSheet.svelte` |
| Media keys, volume flyout, lock-screen tile | `souvlaki` (SMTC on Windows) | `shell/media.rs` |
| Taskbar progress | `pyxee_shell::progress` | `shell::on_state` (Tauri `set_progress_bar`) |
| Thumbnail buttons, jump list | `pyxee_shell::winui` | `shell/winextras.rs`, **opt-in** `--features win-extras` (raw COM, not compiled in the sandbox) |
| Tray, close to tray, single instance, start with Windows, toast | `pyxee_shell::{settings,cli}` | `shell/tray.rs`, `shell/mod.rs`, Tauri plugins `single-instance`, `autostart`, `notification` |
| Tray flyout | `pyxee_shell::anchor` (above a bottom taskbar, below a top one, beside a side one, clamped to the work area, multi-monitor, ignores the click that blurred it) | `shell/flyout.rs`, `views/Flyout.svelte` |
| Mini player | `pyxee_shell::geometry` (position kept reachable when monitors change) | `shell/mini.rs`, `views/Mini.svelte` |
| Sleep/resume, network changes | `pyxee_shell::power` (clock gap = slept; local address / route change = network change; what to do) | `shell/power.rs` (polls every 2 s, no Windows messages) |

One bundle serves three windows: `?view=flyout` and `?view=mini` pick the small views (`ui/src/lib/windows.ts`).

Settings (`shell.json` in the app data folder): close to tray, start with Windows, toast on change, the mini player's
position. Damaged files are kept as `shell.json.bad`. A second launch (or a jump-list task) hands its arguments
(`--toggle --play --pause --next --prev --mini`) to the running player. `--autostart` (added by start-with-Windows)
keeps the window hidden in the tray when close-to-tray is on.

Known limits: the window theme command only drives the main window, so the flyout and mini player follow the Windows
theme for their Mica backdrop while their colours follow the app theme.
