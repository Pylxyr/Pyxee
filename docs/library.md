# Milestone 5: library

## What it stores
`crates/library` keeps everything in one SQLite file (`library.db` in the Local app data folder (`%LOCALAPPDATA%\dev.pyxee.app`), WAL mode, bundled SQLite so
nothing needs installing): liked songs, playlists (create, rename, delete, add, reorder, remove), listening history, and the
data behind Explore's "Tune in" cards and Home's "Recently played". The queue and playback position still live in `session.json` (milestone 2/3).

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

## Covers of songs on this PC

Titles, artists, albums and lengths come from the files' tags (`lofty`, read once per scan). The cover is not stored: every local song's thumbnail is an address in the window's own `pyxeecover` scheme (`http://pyxeecover.localhost/<hex path>?v=<modified>` on Windows), and the app answers it when the window asks (`crates/app/src/local_cover.rs`, `pyxee_sources::local::read_cover`).

- **Where the picture comes from:** the front cover in the file's tags (else the first picture), else a picture beside the song named `cover`, `folder`, `front` or `albumart` (`.jpg`, `.jpeg`, `.png`, `.webp`, any letter case). No picture: 404, and the UI keeps its music-note placeholder.
- **Safety:** the hex path must be an absolute, non-network path to a file the player plays by extension, and only bytes that start like a JPEG, PNG, GIF or WebP leave the app. The CSP lists the scheme under `img-src`.
- **Colours:** the page reads the pixels directly (the answer carries a CORS header); if that fails, `cover_bytes` reads the same picture from the file.
- **Older songs:** songs saved before covers were read (queue, playlists, likes, history) have no thumbnail of their own; `TrackDto` gives every `file:` song without one the same address on its way to the window (`own_or_local_cover` in `dto.rs`).
- **Cost:** nothing at scan time, nothing for a song that is not on screen; a cover is read when its row or the player shows it, and the webview caches it (the address changes when the file does).

## When songs on this PC are removed, moved or their drive is gone

| What happened | What Pyxee does |
|---|---|
| A file was deleted or moved out of a chosen folder | The next scan (at start, "Rescan") takes it out of the Songs list and says how many went. Playlists, likes, history and the queue keep it: their rows are dimmed and read "File missing · Artist" (`availability.svelte.ts`, one `local_missing` call per list; coming back to the window looks again, at most every 5 s). |
| A chosen folder cannot be listed (drive unplugged, network location down, folder renamed or deleted) | The scan **keeps** that folder's songs instead of emptying the library (`scan_folders` → `Scan::unreachable`), says which folder, and Settings shows "Can't be reached" on it. Remove it there if it is gone for good. |
| A missing song is played, or reached in the queue | It is skipped with the real reason ("its file is no longer in D:\\Music (moved, renamed or deleted)" or "its folder … can't be found (is the drive or network location connected?)"), not the streaming wording. At most three in a row are skipped; then the player stops on the last one with the same reason (`Failure::FileMissing`). |
| A file disappears while it plays | The open file keeps playing on Windows (deleting does not close it); if the drive itself goes, the song stops early with the same explanation. |
| A song has no file but is a YouTube song | Nothing changes: only `file:` keys are checked. |

What it does not do (yet): look for a moved file by its title and artist, or offer to remove missing songs from a playlist in one go.

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
