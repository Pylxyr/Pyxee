# Pyxee

A standalone music player for Windows 10 (22H2) and Windows 11, modeled on YouTube Music for Android.
Rust (audio, queue, library) + Tauri 2 (WebView2) + Svelte/TypeScript/Tailwind UI. Personal use.

Status: milestones 1-5 done (gapless audio, core state machine, UI shell, real engine + search + queue, library with likes,
playlists, history and Home). The audio, yt-dlp and library parts are covered by automated tests with local files, but have
not yet been run against YouTube, a real sound card or a real Windows profile: see `docs/engine.md` and `docs/library.md` for
what to verify on Windows. Set `PYXEE_SIM=1` to run the UI against the built-in simulator instead.

## Prerequisites (Windows)
Rust 1.91 (MSVC; `rust-toolchain.toml` selects it and rustup installs it on first use), Visual Studio Build Tools (C++ workload), Node 22+ and the WebView2 runtime (preinstalled on Windows 11).
The Tauri CLI comes with the UI's npm packages, so there is nothing else to install (no `cargo install`, no long compile).

## Get yt-dlp
Pyxee needs yt-dlp. You do not install it by hand (audio is decoded inside the app by a built-in libav, so no ffmpeg is needed or started):
```
get-tools.cmd            (same as: cargo xtask tools)
get-tools.cmd --update   refresh to the latest versions
```
This downloads yt-dlp from its official GitHub release, verifies it against the published SHA-256 checksum (nothing is
installed if it does not match) and puts it in `crates/app/binaries`. Development builds find it there automatically. To use
your own copy instead, set `PYXEE_YTDLP` to its path, or put `yt-dlp.exe` next to the app.

### Optional settings (environment variables)
| Variable | Effect |
|---|---|
| `PYXEE_YTDLP` | Path to `yt-dlp`. |
| `PYXEE_COOKIES` | Path to a Netscape-format `cookies.txt`. A logged-in session is much less likely to get HTTP 429 or "sign in to confirm you're not a bot". yt-dlp may rewrite this file. A path that is not an existing file is ignored (with a warning). |
| `PYXEE_COOKIES_FROM_BROWSER` | yt-dlp's `BROWSER[+KEYRING][:PROFILE][::CONTAINER]`, e.g. `firefox`. Ignored if `PYXEE_COOKIES` is usable. Chromium browsers on Windows often lock their cookie store; an exported `cookies.txt` is more reliable. |
| `PYXEE_LOG` | `error`, `warn`, `info` (default), `debug`, `trace`. |

yt-dlp is also paced by the app (one run at a time, 1 s apart, 2 minutes of refusals after a rate limit); see `docs/engine.md`.

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
skipped, seeks, stalls, and any crash. Find it in the app: **Settings** (the gear icon, top right) -> **Diagnostics**
-> **Open log folder** (or copy the path). On Windows the file is
`%LOCALAPPDATA%\dev.pyxee.app\logs\pyxee.log`; when it passes 2 MB the old one becomes `pyxee.log.1`. Times are Taipei time (UTC+8, written like `2026-10-12 21:30:00.123+08:00`).
`dev.cmd` also prints the same lines in its console. For more detail set `PYXEE_LOG=debug` before starting.
If a song misbehaves, send me the lines around its title (search the file for the song's name).

## Themes
Settings (the gear icon, top right) has an Appearance section with a theme picker: Dark, Light, YT Music, OLED black, and six more (Deep blue,
Sky, Graphite and gold, Turquoise, Ember, Paper). The last few are original palettes in the spirit of MusicBee's community
skins, not copies of any skin. **Match system** uses one theme when Windows is light and another when it is dark.
**Customize...** edits the seven colours of a theme with a live preview and a readability check, and saves it as your own;
**Export** copies a theme as text and **Import a theme** reads it back, so themes can be shared like skins.
The native window (title bar, Mica) follows the chosen theme, not only the Windows setting.

## Build the installer
```
build-installer.cmd      (same as: cargo xtask installer)
```
Downloads the tools if missing, builds the UI, then runs `tauri build` with `crates/app/tauri.bundle.conf.json`, which
bundles yt-dlp as a sidecar (installed next to `pyxee-app.exe`) and the licence notices, and makes the installer
download the WebView2 runtime when a machine lacks it. Installers (NSIS `.exe` and `.msi`) end up in `target/release/bundle/`.
They are unsigned, so Windows SmartScreen warns on first run: choose "More info", then "Run anyway". Signing and the portable build come in milestone 7.

## Release
On GitHub: **Actions -> Cut release -> Run workflow -> patch / minor / major**. It runs CI, bumps the version, tags, builds and tests the
installer, writes AI-assisted release notes and publishes. See `docs/releasing.md`.

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

## Checking a build on Windows
`scripts\check-windows.cmd` builds, runs every test, proves no ffmpeg process is ever started (a fake `ffmpeg.exe` sits first on `PATH` and logs
any attempt), and runs live YouTube checks of the real search / resolve / decode / seek / gapless path. It writes one report to
`pyxee-check\` (stream URLs, cookie paths and your user name are redacted). Useful options: `-SkipTests`, `-SkipLive`, `-Ui`,
`-Installer`, `-UpdateYtdlp`, `-CookieFile <cookies.txt>`, `-CookieBrowser firefox`, `-Manual` (answer y/n for things to try in the
app) and `-AnalyzeLog` (summarise the app's own log). Details: the comment block at the top of `scripts\check-windows.ps1`.

## Licences
Pyxee is MIT licensed. This software uses libraries from the FFmpeg project under the LGPLv2.1 (a reduced, audio-decoding-only build,
linked statically; see `scripts/build-ffmpeg.sh`). The installer includes the full licence text and the notices in `licenses/`
(`THIRD_PARTY.txt`, `FFmpeg-LGPL-2.1.txt`); they are also attached to every release. yt-dlp is bundled as a separate program (The Unlicense).
