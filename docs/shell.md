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

## Notification-area mini player (decision)
The goal was a compact player in the taskbar notification area, left of the "show hidden icons" chevron. Windows has no supported way to put an app's own toolbar there: deskbands are gone from the Windows 11 taskbar and unsupported on Windows 10's, and the undocumented ways break between builds. What every Windows version supports, and what Pyxee does:

* **The main tray icon** shows "Title – Artist" when you point at it, and a left click opens the **flyout** (title, artist, progress, previous / play-pause / next), anchored to the icon. Setting "Clicking the Pyxee tray icon opens the mini controls" turns this off, and a click then opens the window. The right-click menu is always there.
* **Optional transport icons** (setting "Show previous, play and next buttons in the notification area"): three more tray icons (previous, play/pause, next) beside the main one. Play/pause shows what pressing it will do. Each press is the same command as a media key. The icons are outlined so they read on a light and a dark taskbar (`assets/logo/make-tray-glyphs.py`).
* Limits that are Windows', not Pyxee's: it decides where new tray icons sit and whether they start in the overflow flyout, and an app cannot pin or order them. The user drags them out once (Windows 11: Settings > Personalization > Taskbar > Other system tray icons) and Windows remembers. The title and artist are in the tooltip and flyout, not printed in the strip, and there is no progress bar in the strip (the taskbar button keeps its own progress bar).

Logic: `pyxee_shell::tray_controls` (glyph, tooltip, 120-character cut) and the settings; glue: `shell/tray_buttons.rs`, `shell/tray.rs`.

Known limits: the window theme command only drives the main window, so the flyout and mini player follow the Windows
theme for their Mica backdrop while their colours follow the app theme.

## Discord Rich Presence

Shows the current song as "Listening to Pyxee" in the Discord desktop client, through Discord's local IPC (no bot token, no OAuth, no network). Code: `crates/app/src/shell/discord.rs`, fed from `shell::on_state`.

- **Setup:** create an application in the Discord Developer Portal (its name is what members see after "Listening to") and set `PYXEE_DISCORD_CLIENT_ID` to its Application ID, at run time or when building. There is no built-in ID. Without a valid one the feature is off and the log says so.
- **Needs:** the Discord desktop app running on the same PC (the web client has no local IPC).
- **Behaviour:** title and artist while playing, with a progress bar when the duration is known; "Paused" without a clock while paused; cleared when idle or on quit. Seeks correct the bar; ordinary position ticks send nothing. A song that is only sitting paused (restored session) is not shown until something plays.
- **Failures:** if Discord is not running or the pipe drops, playback is unaffected. Reconnects back off from 2 to 30 seconds, and only while something is waiting to be shown.
- **Limits:** no cover art, no buttons, no settings switch. A paused presence stays until another song plays or the app quits. Only the Quit path clears the presence on exit.
