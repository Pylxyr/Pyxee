# Where Pyxee keeps its files

| System | Data folder |
|---|---|
| Windows | `%LOCALAPPDATA%\dev.pyxee.app`, or `data\` next to `Pyxee.exe` for a portable copy |
| Linux | `~/.local/share/dev.pyxee.app` |

It holds `library.db` (your likes, playlists and listening history), `session.json` (your queue and where you were), `shell.json` (settings such as the tray, starting with your computer and update checks), `local-library.json` (the music folders you added), `audio-cache.json` (the size you chose for kept songs), `tools\` (the updated yt-dlp) and, only when logging has been turned on,
`logs\last-panic.txt` (written only if the app panics, even with logging off; attach it to an issue) and `logs\pyxee.log` (the log; **Settings** -> **Diagnostics** -> **Open log folder** opens it, and that section is hidden
while logging is off, which is the default). `session.json` is rewritten when something changes (a pause, a skip, a queue
change, closing or quitting), not on a timer while a song plays.

Songs kept for playing offline (**Library** -> **Offline**, off until you pick a size) can take up to 4 GB, so an installed copy does not keep them in the data folder: they are in the `audio` folder of the system's cache folder for Pyxee (`%LOCALAPPDATA%\dev.pyxee.app\audio` on Windows, `~/.cache/dev.pyxee.app/audio` on Linux). A portable copy keeps them in `data\audio` beside the program, so nothing is left in your profile (songs that an earlier portable version kept in the system cache folder stay there; you can delete that folder). **Remove kept songs** on that page empties the folder in use.

Pyxee has no advanced settings: it always runs with its built-in defaults (no sign-in cookies, its own copy of yt-dlp,
no log). Environment variables that change how it behaves exist only for developers and are read by
developer builds only; see [development.md](development.md).
