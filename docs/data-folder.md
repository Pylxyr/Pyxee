# Where Pyxee keeps its files

| System | Data folder |
|---|---|
| Windows | `%LOCALAPPDATA%\dev.pyxee.app`, or `data\` next to `Pyxee.exe` for a portable copy |
| Linux | `~/.local/share/dev.pyxee.app` |

It holds `library.db` (your likes, playlists and listening history), `session.json` (your queue and where you were), `shell.json` (settings such as the tray, starting with your computer and update checks), `ui-prefs.json` (a copy of how you set up the look of the app, see below), `local-library.json` (the music folders you added), `audio-cache.json` (the size you chose for kept songs), `tools\` (the updated yt-dlp) and, only when logging has been turned on,
`logs\last-panic.txt` (written only if the app panics, even with logging off; attach it to an issue) and `logs\pyxee.log` (the log; **Settings** -> **Diagnostics** -> **Open log folder** opens it, and that section is hidden
while logging is off, which is the default). `session.json` is rewritten when something changes (a pause, a skip, a queue
change, closing or quitting), not on a timer while a song plays.

yt-dlp's own cache (what it works out about a player between runs) is the `yt-dlp` folder next to it: Pyxee tells yt-dlp where to keep it, so nothing goes to `%USERPROFILE%\.cache\yt-dlp` (or `~/.cache/yt-dlp`) any more. A portable copy keeps it in its own `cache\yt-dlp` folder beside the program, next to `data\` and not inside it, so it can be deleted at any time without touching your library or settings. An older Pyxee did not do this and may have left a `.cache\yt-dlp` folder in your profile; you can delete it. If no folder can be found at all, yt-dlp is told to keep no cache.

Songs kept for playing offline (**Library** -> **Offline**, off until you pick a size) can take up to 4 GB, so an installed copy does not keep them in the data folder: they are in the `audio` folder of the system's cache folder for Pyxee (`%LOCALAPPDATA%\dev.pyxee.app\audio` on Windows, `~/.cache/dev.pyxee.app/audio` on Linux). A portable copy keeps them in `data\audio` beside the program, so nothing is left in your profile (songs that an earlier portable version kept in the system cache folder stay there; you can delete that folder). **Remove kept songs** on that page empties the folder in use.

`session.json`, `shell.json` and `ui-prefs.json` carry a `version`. If you go back to an older Pyxee, a file written by a newer one is not used and not overwritten: it is renamed to `session.json.v2` (the number is its version) and the older Pyxee starts that file from its defaults. A file that cannot be read is renamed to `.bad` in the same way, and the app starts that file from its defaults.

Your settings survive an update, a reinstall and a reset of the web view. The equalizer, the tray, starting with your computer and the other window settings are in `shell.json`. What the page itself keeps (the theme and your own themes, the Look, the interface size, volume, crossfade, the seek bar style and the search filter) lives in the web view's own storage, which is not in this folder, so Pyxee also keeps a copy in `ui-prefs.json`. It is written a moment after you change one of those settings (not on a timer, and not at all if nothing changed), and when Pyxee starts it puts back whatever the web view no longer has; a setting the web view does have is never replaced. The first start of a version that has this file copies your current settings into it. Only entries the page names with its own `pyxee.` prefix are kept (at most 64 entries, 1 MB in all). The file holds no queue, library or account data. Deleting it only costs the copy. Moving or deleting the data folder takes the copy with it (a portable copy keeps both in `data\`).

Pyxee has no advanced settings: it always runs with its built-in defaults (no sign-in cookies, its own copy of yt-dlp,
no log). Environment variables that change how it behaves exist only for developers and are read by
developer builds only; see [development.md](development.md).
