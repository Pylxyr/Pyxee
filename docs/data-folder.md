# Where Pyxee keeps its files

| System | Data folder |
|---|---|
| Windows | `%LOCALAPPDATA%\dev.pyxee.app`, or `data\` next to `Pyxee.exe` for a portable copy |
| Linux | `~/.local/share/dev.pyxee.app` |

It holds `session.json` (your library and queue), `tools\` (the updated yt-dlp) and, only when logging has been turned on,
`logs\pyxee.log` (the log; **Settings** -> **Diagnostics** -> **Open log folder** opens it, and that section is hidden
while logging is off, which is the default). `session.json` is rewritten when something changes (a pause, a skip, a queue
change, closing or quitting), not on a timer while a song plays.

Pyxee has no advanced settings: it always runs with its built-in defaults (no sign-in cookies, its own copy of yt-dlp,
no log). Environment variables that change how it behaves exist only for developers and are read by
developer builds only; see [development.md](development.md).
