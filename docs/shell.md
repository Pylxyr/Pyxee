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

* **A hidden window's page sleeps.** Hiding a window (close to tray, the flyout, the mini player) or minimizing the main one does not tell WebView2, which then keeps painting and holding memory for a page nobody sees (steady CPU and GPU use under "WebView2 Manager" in Task Manager). `shell/webview_power.rs` marks the page hidden, asks WebView2 for its low-memory level and suspends it, and undoes that when the window is shown, restored or focused. Music is unaffected (Rust plays it). A sleeping page can miss pushed state, so the player, library and sleep-timer stores read it again when the page becomes visible (`visibilitychange`). Sleeping windows are also skipped when the backend pushes state (`webview_power::emit_awake`; the UI listens per window so the skip applies), and the result of `TrySuspend` is logged, so a refusal by WebView2 shows in the log instead of being assumed. At debug level (`PYXEE_LOG=debug`) the log also shows `IsSuspended` right after each `TrySuspend` and again 5 s later; a page that woke up by itself is reported with the events the app sent to all windows meanwhile (`emit_noted`), so the cause can be found. How to measure the effect in Task Manager: `docs/measuring.md`.

Known limits: the window theme command only drives the main window, so the flyout and mini player follow the Windows
theme for their Mica backdrop while their colours follow the app theme.

## Discord Rich Presence

Shows the current song as "Listening to Pyxee" in the Discord desktop client, through Discord's local IPC (no bot token, no OAuth, no network). Code: `crates/app/src/shell/discord.rs`, fed from `shell::on_state`.

- **Setup:** releases are built with the Discord application's ID baked in (the `PYXEE_DISCORD_CLIENT_ID` build variable, see `releasing.md`); its name is what members see after "Listening to". Users cannot change it: there is no setting or run-time variable for it. A build without a valid ID has the feature off and the log says so.
- **Needs:** the Discord desktop app running on the same PC (the web client has no local IPC).
- **Behaviour:** like Spotify's: "Listening to Pyxee" with the title, "by" the artist, the cover art and a progress bar when the duration is known. Pausing takes the activity down; resuming puts it back with a fresh bar. Seeks correct the bar; ordinary position ticks send nothing. While a song loads it is shown without a bar. Cleared when idle or on quit.
- **Cover art:** the song's thumbnail is used when it is a short `https` URL (YouTube thumbnails are). Local files and anything else are skipped, and Discord shows the application's icon instead, so upload an app icon in the Developer Portal.
- **Failures:** if Discord is not running or the pipe drops, playback is unaffected. Reconnects back off from 2 to 30 seconds, and only while something is waiting to be shown.
- **Switch:** Settings > Windows > "Show what I'm playing on Discord" (`discordPresence` in `shell.json`). On by default, so a build with an application ID shows presence as before. Turning it off clears the presence at once and stops the Discord thread; turning it on starts it again, and the next position tick publishes the current song. With no application ID configured the switch has no effect.
- **Limits:** no album name (Pyxee has none to send) and no buttons. Only the Quit path clears the presence on exit.
