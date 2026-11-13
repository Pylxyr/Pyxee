# Development

Everything for building, running and testing Pyxee from source. Users: see the [README](../README.md) and [advanced.md](advanced.md) (the settings); Linux: [linux.md](linux.md).

Rust (audio, queue, library) + a Flutter desktop UI that loads the Rust core through `crates/bridge`
([flutter-ui.md](flutter-ui.md)).

Status: the audio, yt-dlp and library parts are covered by automated tests with local files, but have not all been run
against YouTube, a real sound card or a real Windows profile: see `engine.md` and `library.md` for what to verify on
Windows, and [windows-checklist.md](windows-checklist.md). What the Tauri version had that this one does not yet is
listed in [migration.md](migration.md).

## Prerequisites (Windows)
- Rust 1.91 or newer, MSVC target (`rust-toolchain.toml` selects 1.91 and rustup installs it on first use), with clippy and rustfmt.
- Visual Studio 2022 Build Tools, workload "Desktop development with C++" (MSVC, Windows 10/11 SDK).
- MSYS2 with `make nasm diffutils pkgconf` (builds the minimal FFmpeg): `C:\msys64\usr\bin\bash.exe -lc "pacman -S --noconfirm --needed make nasm diffutils pkgconf"`.
- Flutter 3.44.0 or newer (Dart 3.12 or newer) with Windows desktop enabled: `flutter config --enable-windows-desktop`, then `flutter doctor`.

## Get yt-dlp
Pyxee needs yt-dlp. You do not install it by hand (audio is decoded inside the app by a built-in libav, so no ffmpeg is needed or started):
```
cargo xtask tools            download yt-dlp
cargo xtask tools --update   refresh to the latest versions
```
This downloads yt-dlp from its official GitHub release, verifies it against the published SHA-256 checksum (nothing is
installed if it does not match) and puts it in `tools`. Development builds find it there automatically. To use
your own copy instead, set `PYXEE_YTDLP` to its path, or put `yt-dlp.exe` next to the app.

### Optional settings (environment variables)
The same variables can be set in **Settings** > **Advanced settings**, which saves them to `.env` in the data folder (see [advanced.md](advanced.md)); a variable set in the system wins over that file. `crates/shell/src/envfile.rs` reads it at start-up.

| Variable | Effect |
|---|---|
| `PYXEE_YTDLP` | Path to `yt-dlp`. |
| `PYXEE_JS_RUNTIME` | The JavaScript runtime passed to yt-dlp as `--js-runtimes`: `deno`, `node`, `quickjs` or `bun`, optionally `runtime:path` (a binary or its folder), or `none` to pass nothing. Unset: Deno, then Node, from `PATH`. |
| `PYXEE_COOKIES` | Path to a Netscape-format `cookies.txt`. A logged-in session is much less likely to get HTTP 429 or "sign in to confirm you're not a bot". yt-dlp may rewrite this file. A path that is not an existing file is ignored (with a warning). |
| `PYXEE_COOKIES_FROM_BROWSER` | yt-dlp's `BROWSER[+KEYRING][:PROFILE][::CONTAINER]`, e.g. `firefox`. Ignored if `PYXEE_COOKIES` is usable. Chromium browsers on Windows often lock their cookie store; an exported `cookies.txt` is more reliable. |
| `PYXEE_LOG` | `error`, `warn`, `info`, `debug` or `trace` set how much is printed to the console; `off` prints nothing; the default is `warn`. There is no log file in this version. |
| `PYXEE_DISCORD_CLIENT_ID` | Build time only (set it when you run `cargo build -p pyxee-bridge`): the Discord application ID (17 to 20 digits) baked into the build, which turns on Rich Presence ("Listening to Pyxee" with the current song). Unset: the feature is off. Not read at run time. |
| `PYXEE_UPDATE_PUBLIC_KEY` | Build time only: the public key that checks the signature of the release's `SHA256SUMS.txt`, baked into the build. A build that has it installs only releases whose checksum list is signed with the matching key. Unset: a warning is logged, the checksum list is not authenticated, and the installer's SHA-256 is still checked. See [releasing.md](releasing.md). |

yt-dlp is also paced by the app (one run at a time, 1 s apart, 2 minutes of refusals after a rate limit, doubling each time it repeats, up to 30 minutes); see `engine.md`.

### yt-dlp and a JavaScript runtime (recommended, not required)
Recent yt-dlp prints a warning that YouTube extraction without an external JavaScript runtime is deprecated, and some
formats may disappear without one. This is yt-dlp's requirement, not Pyxee's: the app binary stays pure MSVC and does not
bundle, start or need a runtime itself. The `yt-dlp.exe` that `get-tools` downloads already contains yt-dlp's solver
scripts (EJS), so only a runtime is missing. Either of these silences the warning (see yt-dlp's
[EJS wiki page](https://github.com/yt-dlp/yt-dlp/wiki/EJS)):

* **Deno 2.x on `PATH`** (the one yt-dlp enables by default): `winget install DenoLand.Deno`, then open a new terminal (and
  sign out/in, or restart the app launcher, for the installed app to see the new `PATH`).
* **Node, which developers here already have**: nothing to configure. Pyxee starts yt-dlp with `--ignore-config`, so a
  `--js-runtimes` line in `%APPDATA%\yt-dlp\config` is **not** read by Pyxee's runs. Instead Pyxee looks for `deno`, then
  `node`, on `PATH` when it starts and passes `--js-runtimes deno` or `--js-runtimes node` itself (nothing is added when it
  finds neither). To choose another runtime or a path, set `PYXEE_JS_RUNTIME` (below). Needs a recent yt-dlp
  (`cargo xtask tools --update`); an old one rejects the option. Pyxee does not run the runtime to check its version.

`scripts\check-windows.cmd` reports which of these it found (`ytdlp-js-runtime`).

## Run
In PowerShell (use separate lines; PowerShell 5 has no `&&`). First time only, from the repo root: build the minimal FFmpeg
(slow; from a shell with the MSVC environment, such as "x64 Native Tools Command Prompt for VS 2022") and download yt-dlp.
```
cargo xtask ffmpeg
cargo xtask tools
```
Then build the bridge library and start the app:
```
cargo build -p pyxee-bridge --release
cd ui_flutter
flutter pub get
flutter run -d windows
```
The Flutter build only copies `target\release\pyxee_bridge.dll` next to the app (`ui_flutter/hook/build.dart` does not run
cargo), so run the `cargo build` line again after every Rust change. `cargo xtask tools` puts yt-dlp in `tools\`; a
release-built bridge does not look there by itself, so if the app says yt-dlp was not found, set `PYXEE_YTDLP` to the file's
full path or put `yt-dlp.exe` next to the app. To use FFmpeg libraries built elsewhere set `PYXEE_FFMPEG_DIR`.

## Build a release and the installer
On Windows, `cargo xtask installer` builds the Rust bridge and the Flutter app (`flutter build windows --release`, which leaves
the app in `ui_flutter\build\windows\x64\runner\Release`), gathers it with `yt-dlp.exe`, the Visual C++ runtime and the
licence texts in `target\stage\Pyxee`, and writes the NSIS installer to `target\installer`. `cargo xtask portable` writes the
portable zip to `target\portable`, `cargo xtask stage` stops after gathering, and `cargo xtask smoke` starts a staged or installed
copy. The installer needs NSIS (`makensis`; `PYXEE_MAKENSIS` if it is not in its default folder). Signing, the installer script and
how the release workflow uses all this are in [releasing.md](releasing.md).

## Test
Run each line on its own (PowerShell 5 does not accept `&&`):
```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd ui_flutter
flutter analyze
flutter test
```
`cargo test` includes the engine tests (`crates/bridge/src/engine_tests.rs`). `flutter test` runs `ui_flutter/test/`.

## Dependency checks
CI has a `deny` job (cargo-deny, advisories only). Run it locally with `cargo deny --locked check advisories`. Advisories are
not blocking (`continue-on-error`) while the baseline is not clean.

## Layout
`crates/core` queue + state machine (no I/O) · `crates/audio` decode, resample, gapless, output · `crates/sources` yt-dlp, local ·
`crates/library` SQLite library · `crates/lyrics` lyrics · `crates/shell` decisions that need no window (sleep timer, update checks, the
`.env` file) · `crates/net` HTTP defaults (certificate store, one retry for a stale connection) · `crates/bridge` the C interface the
Flutter app loads, with the engine, the library service, the session and the contract types · `crates/spike` milestone 1 harness ·
`ui_flutter/` the Flutter app · `xtask/` tool downloader and build helpers · `licenses/` licence texts · `tools/` downloaded yt-dlp
(git-ignored) · `packaging/windows/` the NSIS installer script · `scripts/` version, release-notes and check scripts · `assets/` logo and icons.

## Checking a build on Windows
`scripts\check-windows.cmd` builds and tests everything (`cargo fmt`, clippy, `cargo test`, the Flutter analysis, tests and a release
build), proves no ffmpeg process is started, runs live YouTube checks and writes one report. With `-Installer` it also builds the
installer and the portable zip, starts the staged app, installs the installer silently into a temporary folder, starts that and
uninstalls it; `-Ci` replays the release workflow's build steps in their order. `scripts\test-build.ps1` is the short version: it
builds the bridge and runs the app with `flutter run`.

## Linux
The Rust workspace builds, passes clippy and passes its tests on Linux (the `linux` job in CI); Windows-only code is behind
`cfg(windows)`. System packages (Debian/Ubuntu): `apt install pkg-config nasm make libasound2-dev`; Arch: `pacman -S --needed base-devel git nasm alsa-lib`.
```
cargo xtask ffmpeg          # the minimal static FFmpeg, once (needs git, make, nasm, gcc)
cargo test --workspace
cargo build -p pyxee-bridge --release   # target/release/libpyxee_bridge.so
```
The Flutter app on Linux and a Linux package are not set up yet: [linux.md](linux.md).
