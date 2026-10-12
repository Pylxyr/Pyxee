# Pyxee

A standalone music player for Windows 10 (22H2) and Windows 11, modeled on YouTube Music for Android.
Rust (audio, queue, library) + Tauri 2 (WebView2) + Svelte/TypeScript/Tailwind UI. Personal use.

Status: milestones 1-5 done (gapless audio, core state machine, UI shell, real engine + search + queue, library with likes,
playlists, history and Home). The audio, yt-dlp and library parts are covered by automated tests with local files, but have
not yet been run against YouTube, a real sound card or a real Windows profile: see `docs/engine.md` and `docs/library.md` for
what to verify on Windows. Set `PYXEE_SIM=1` to run the UI against the built-in simulator instead.

## Prerequisites (Windows)
Rust (stable, MSVC), Visual Studio Build Tools (C++ workload), Node 22+ and the WebView2 runtime (preinstalled on Windows 11).
The Tauri CLI comes with the UI's npm packages, so there is nothing else to install (no `cargo install`, no long compile).

## Get yt-dlp and ffmpeg
Pyxee needs both. You do not install them by hand:
```
get-tools.cmd            (same as: cargo xtask tools)
get-tools.cmd --update   refresh to the latest versions
```
This downloads yt-dlp and an LGPL build of ffmpeg from their official GitHub releases, verifies each against the published
SHA-256 checksum (nothing is installed if it does not match) and puts them in `crates/app/binaries`. Development builds find
them there automatically. To use your own copies instead, set `PYXEE_YTDLP` / `PYXEE_FFMPEG` to their paths, or put
`yt-dlp.exe` / `ffmpeg.exe` next to the app. Download size is about 190 MB.

## Run
Double-click `dev.cmd` (or run it from a terminal). It installs the UI packages and downloads the tools if they are missing,
starts the UI dev server in its own window, waits for it, and launches the desktop app.

By hand, in PowerShell (use separate lines; PowerShell 5 has no `&&`):
```
npm install --prefix ui
npm run dev --prefix ui            # leave this running in one terminal
```
and in a second terminal:
```
cd crates\app
..\..\ui\node_modules\.bin\tauri dev
```
Without Tauri, `npm run dev --prefix ui` also works in a browser against a built-in mock backend.

## Logs
Pyxee writes a log of what it does: every song it starts, how the stream was resolved and opened, why a song failed or was
skipped, seeks, stalls, and any crash. Find it in the app: **Appearance** (the compass icon, top right) -> **Diagnostics**
-> **Open log folder** (or copy the path). On Windows the file is
`%APPDATA%\dev.pyxee.app\logs\pyxee.log`; when it passes 2 MB the old one becomes `pyxee.log.1`. Times are UTC.
`dev.cmd` also prints the same lines in its console. For more detail set `PYXEE_LOG=debug` before starting.
If a song misbehaves, send me the lines around its title (search the file for the song's name).

## Build the installer
```
build-installer.cmd      (same as: cargo xtask installer)
```
Downloads the tools if missing, builds the UI, then runs `tauri build` with `crates/app/tauri.bundle.conf.json`, which
bundles yt-dlp and ffmpeg as sidecars (installed next to `pyxee-app.exe`) and the licence notices, and makes the installer
download the WebView2 runtime when a machine lacks it. Installers (NSIS `.exe` and `.msi`) end up in `target/release/bundle/`.
They are unsigned, so Windows SmartScreen warns on first run: choose "More info", then "Run anyway". The installer is large
(roughly 60-80 MB) because ffmpeg is about 130 MB uncompressed. Signing and the portable build come in milestone 7.

## Test
Run each line on its own (PowerShell 5 does not accept `&&`):
```
npm run check --prefix ui
npm test --prefix ui
npm run build --prefix ui          # Tauri embeds ui/dist
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
`cargo test -p pyxee-app` regenerates the TypeScript types in `ui/src/lib/bindings` from the Rust types.

## Layout
`crates/core` queue + state machine (no I/O) · `crates/audio` decode, resample, gapless, output · `crates/sources` yt-dlp, local ·
`crates/library` SQLite library ·
`crates/app` Tauri glue · `crates/spike` milestone 1 harness · `ui/` Svelte frontend · `xtask/` tool downloader and installer build.
The in-app yt-dlp updater with rollback, the portable build and code signing come in milestone 7.
