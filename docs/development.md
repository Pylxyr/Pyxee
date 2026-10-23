# Development

Everything for building, running and testing Pyxee from source. Users: see the [README](../README.md).

Rust (audio, queue, library) + Tauri 2 (WebView2) + Svelte/TypeScript/Tailwind UI.

Status: milestones 1-5 done (gapless audio, core state machine, UI shell, real engine + search + queue, library with likes,
playlists, history and Home). The audio, yt-dlp and library parts are covered by automated tests with local files, but have
not yet been run against YouTube, a real sound card or a real Windows profile: see `engine.md` and `library.md` for
what to verify on Windows. Set `PYXEE_SIM=1` to run the UI against the built-in simulator instead.

## Prerequisites (Windows)
Rust 1.91 (MSVC; `rust-toolchain.toml` selects it and rustup installs it on first use), Visual Studio Build Tools (C++ workload), Node 22+ and the WebView2 runtime (preinstalled on Windows 11).
The Tauri CLI comes with the UI's npm packages, so there is nothing else to install (no `cargo install`, no long compile).

## Get yt-dlp
Pyxee needs yt-dlp. You do not install it by hand (audio is decoded inside the app by a built-in libav, so no ffmpeg is needed or started):
```
cargo xtask tools            download yt-dlp
cargo xtask tools --update   refresh to the latest versions
```
This downloads yt-dlp from its official GitHub release, verifies it against the published SHA-256 checksum (nothing is
installed if it does not match) and puts it in `crates/app/binaries`. Development builds find it there automatically. To use
your own copy instead, set `PYXEE_YTDLP` to its path, or put `yt-dlp.exe` next to the app.

### Optional settings (environment variables)
The same variables can be set in **Settings** > **Advanced**, which saves them to `.env` in the data folder (see the README); a variable set in Windows wins over that file. `crates/shell/src/envfile.rs` reads it at start-up.

| Variable | Effect |
|---|---|
| `PYXEE_YTDLP` | Path to `yt-dlp`. |
| `PYXEE_JS_RUNTIME` | The JavaScript runtime passed to yt-dlp as `--js-runtimes`: `deno`, `node`, `quickjs` or `bun`, optionally `runtime:path` (a binary or its folder), or `none` to pass nothing. Unset: Deno, then Node, from `PATH`. |
| `PYXEE_COOKIES` | Path to a Netscape-format `cookies.txt`. A logged-in session is much less likely to get HTTP 429 or "sign in to confirm you're not a bot". yt-dlp may rewrite this file. A path that is not an existing file is ignored (with a warning). |
| `PYXEE_COOKIES_FROM_BROWSER` | yt-dlp's `BROWSER[+KEYRING][:PROFILE][::CONTAINER]`, e.g. `firefox`. Ignored if `PYXEE_COOKIES` is usable. Chromium browsers on Windows often lock their cookie store; an exported `cookies.txt` is more reliable. |
| `PYXEE_LOG` | `error`, `warn`, `info` (default), `debug`, `trace`. |
| `PYXEE_DISCORD_CLIENT_ID` | Build time only: the Discord application ID (17 to 20 digits) baked into the build, which turns on Rich Presence ("Listening to Pyxee" with the current song). Unset: the feature is off. Not read at run time. See `shell.md`. |

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
In PowerShell (use separate lines; PowerShell 5 has no `&&`). First time only: install the UI packages and download yt-dlp.
```
npm install --prefix ui
cargo xtask tools
```
Then start the UI dev server and leave it running in one terminal:
```
npm run dev --prefix ui
```
and in a second terminal:
```
cd crates\app
..\..\ui\node_modules\.bin\tauri dev
```
Without Tauri, `npm run dev --prefix ui` also works in a browser against a built-in mock backend.

## Build the installer
```
cargo xtask installer
```
Downloads the tools if missing, builds the UI, then runs `tauri build` with `crates/app/tauri.bundle.conf.json`, which
bundles yt-dlp as a sidecar (installed next to `pyxee-app.exe`) and the licence notices, and makes the installer
download the WebView2 runtime when a machine lacks it. Installers (NSIS `.exe` and `.msi`) end up in `target/release/bundle/`.
Until a code-signing certificate is configured they are unsigned, so Windows SmartScreen warns on first run: choose "More info", then "Run anyway". `cargo xtask portable` builds a portable zip (everything stays in a `data` folder beside `Pyxee.exe`). Both are described in `releasing.md`, which also says how signing is switched on.

## Release
On GitHub: **Actions -> Cut release -> Run workflow -> patch / minor / major**. It runs CI, bumps the version, tags, builds and tests the
installer, writes AI-assisted release notes and publishes. See `releasing.md`.

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

## Dependency checks
CI has a `deny` job (`deny.toml`, cargo-deny) and an `npm audit --omit=dev` step. Run them locally with `cargo deny --locked check` and `npm audit --omit=dev --prefix ui`.

* **Licences, bans, sources: blocking.** Every licence in the shipped Windows build is on the allow-list in `deny.toml`, each with the reason; a new licence fails the job until someone reads it and adds it there (and to `about.toml`).
* **Advisories: not blocking** (`continue-on-error`), because the baseline is not clean and fixing it means upgrading dependencies. Baseline on 2026-10-22 (cargo-deny 0.20.2, RustSec database of that day): `rustls` 0.23.45 (RUSTSEC-2026-0346, -0347, -0348, -0349), `rustls-webpki` (RUSTSEC-2026-0382), `webpki-roots` (RUSTSEC-2026-0381), `rustls-pemfile` (RUSTSEC-2025-0134, unmaintained), `zip` 2.4.2 (RUSTSEC-2026-0367, -0368, -0369), `ringbuf` 0.4.8 (RUSTSEC-2026-0293). Nothing has been judged harmless here; the next dependency update should clear them, after which the advisories step can become blocking.
* **npm audit: informational.** Zero vulnerabilities in the runtime dependencies on 2026-10-22.

**Notices.** `crates/app/licenses/THIRD_PARTY_DEPENDENCIES.txt` (licence notices of every shipped Rust crate and npm package) is generated, never edited: `python scripts/gen_dependency_notices.py` (needs `cargo install cargo-about --locked --version 0.9.2` and `npm ci --prefix ui`; add `--check` to diff without writing). It records the hashes of `Cargo.lock` and `ui/package-lock.json`; `scripts/tests/test_licenses.py` fails when either lock file changed without regenerating it, so run the script whenever dependencies change and commit the result.

## Layout
`crates/core` queue + state machine (no I/O) · `crates/audio` decode, resample, gapless, output · `crates/sources` yt-dlp, local ·
`crates/library` SQLite library ·
`crates/app` Tauri glue · `crates/spike` milestone 1 harness · `ui/` Svelte frontend · `xtask/` tool downloader and installer build.
Settings > Updates checks for a newer Pyxee (GitHub releases, installer checked against `SHA256SUMS.txt`) and updates yt-dlp in place (checksum-verified,
test-run, then swapped in; a failure changes nothing). A portable copy is updated by downloading the new zip instead.

## Checking a build on Windows
`scripts\check-windows.cmd` builds, runs every test, proves no ffmpeg process is ever started (a fake `ffmpeg.exe` sits first on `PATH` and logs
any attempt), and runs live YouTube checks of the real search / resolve / decode / seek / gapless path. It writes one report to
`pyxee-check\` (stream URLs, cookies and tokens, your user and computer name and your profile, AppData and Temp folders are redacted, and a self-test proves
that first in every run). The default run is the full one: formatting, clippy, every test (with `--locked`), the UI checks and the live YouTube checks; a live check that
cannot run (no FFmpeg libraries, rate limited) is a failure, not a skip, and any FAIL gives a non-zero exit code. Options: `-Installer` (also builds and inspects the installer and the portable zip), `-Ci` (replays the release workflow's portable step the way a clean runner meets it, including MSYS2's first-ever login),
`-UpdateYtdlp`, `-SelfTest` (only the redaction test), and, not recommended, `-SkipTests` / `-SkipLive` (a run with either does not prove the build), `-CookieFile <cookies.txt>`, `-CookieBrowser firefox`, `-Manual` (answer y/n for things to try in the
app) and `-AnalyzeLog` (summarise the app's own log). Details: the comment block at the top of `scripts\check-windows.ps1`. To measure idle CPU, GPU and memory while Pyxee is hidden, see `docs/measuring.md`.
