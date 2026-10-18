# Milestone 5: library

## What it stores
`crates/library` keeps everything in one SQLite file (`library.db` in the Local app data folder (`%LOCALAPPDATA%\dev.pyxee.app`), WAL mode, bundled SQLite so
nothing needs installing): liked songs, playlists (create, rename, delete, add, reorder, remove), listening history, and the
data behind Home's quick picks. The queue and playback position still live in `session.json` (milestone 2/3).

Every method takes the current time as an argument and nothing reads a clock, so the behaviour is deterministic and tested.

## Rules worth knowing
- Playlist names are trimmed and must be 1-100 characters (counted as characters, not bytes).
- Adding songs to a playlist skips ones already there and tells you how many were new; creating a playlist from the queue
  keeps the queue as it is, repeats included.
- Delete, remove-from-playlist and clear-history all return exactly what is needed to undo them. The UI shows an in-page
  snackbar with Undo (never a native dialog); undo restores order, ids and timestamps.
- A play counts toward history after the listener has really heard half the song or 30 seconds, whichever is less. Skipping
  through songs, or seeking to the end, does not count (`PlayCounter` in core, fed by what became audible).
- Quick picks score songs by plays in the last 30 days (2 points each), being liked (6), and a small recency bonus. Songs that
  are neither played recently nor liked are left out. "Recently played" is each song once, by its latest play.
- If `library.db` cannot be opened (corrupt, read-only), the app runs with an in-memory library and shows a banner:
  "Your library couldn't be opened (...). Likes, playlists and history won't be saved this session." It never silently drops data.

## How the UI stays in sync
After any change Rust emits `library-changed` with a snapshot (revision, liked keys, playlist summaries, history count). The UI
store mirrors it; detail views (Liked songs, a playlist, History, Home) reload when the revision changes. Commands reject with a
plain sentence that the UI shows as is.

## Verified here
Library crate: 22 tests, including two deliberate breakages (drag-down shifting the wrong way, never-played songs entering quick
picks) that the tests caught. Service layer: DTO round trips for every undo. UI: 118 tests covering likes, the playlist
picker, name prompts and their errors, playlist reorder (buttons and drag) and undo, save-queue-as-playlist, history and its
undo, Home, and the library-error banner.

## Not verified yet (needs your PC)
Real SQLite file behaviour on Windows (path, antivirus, the one-time move from Roaming, see below), and the history threshold against real
listening. Likes are keyed by the track key (`yt:<id>`), so the same song uploaded twice is two songs.
Playlist import/export and syncing are out of scope for v1.

## Where the files live, and the move from Roaming

Everything (library, `session.json`, `shell.json`, `logs/`) is in `%LOCALAPPDATA%\dev.pyxee.app`. Versions up to 0.0.2 used
`%APPDATA%\dev.pyxee.app` (Roaming), where roaming profiles, folder redirection and backup tools can sync a live SQLite file and
its `-wal`/`-shm` side files half-way, which corrupts it. On the first start of a newer version the library (with its side
files), the session and the settings are **copied** over (`pyxee_shell::datadir`, tested on any OS):
- nothing is overwritten: once the Local folder has a `library.db`, the old folder is never looked at again;
- each file is copied under a temporary name and renamed, and its size is checked, so there is never a truncated database;
- if the library cannot be copied, the app keeps using the old folder for everything and tries again next start (the log says
  `data folder: could not copy ...`), so a failed move never shows an empty library;
- the originals stay in Roaming as a backup; delete `%APPDATA%\dev.pyxee.app` yourself once you are happy. Old logs are not moved.
