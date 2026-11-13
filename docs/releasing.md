# Releasing

Pyxee is two builds that are shipped together: the Rust bridge (`pyxee_bridge.dll`, with FFmpeg's libav linked in) and the
Flutter window (`ui_flutter`) that loads it. `cargo xtask` gathers them, `yt-dlp.exe`, the Visual C++ runtime and the notices
in one folder (`target\stage\Pyxee`), and turns that folder into the installer and the portable zip.

## One click

1. On GitHub: **Actions -> Cut release -> Run workflow**. Leave the branch on `main`.
2. Pick **patch**, **minor** or **major** (or type an exact version such as `0.2.0` or `0.2.0-rc1`; a `-` makes it a pre-release) and run it. On a release candidate, **patch** releases it (`0.5.1-rc1` becomes `0.5.1`, `0.6.0-rc3` becomes `0.6.0`); type an exact version such as `0.6.0-rc4` for the next candidate.

That is all. The workflow then:

1. runs the full CI on `main` (`cargo fmt`, clippy and tests on Windows and Linux, the Flutter analysis and a release build of the app, the release scripts' own tests). If anything fails, nothing is bumped or tagged;
2. works out the next version, writes it into every file that carries it, commits `Release X.Y.Z` and pushes the commit and the tag `vX.Y.Z` in one atomic push (if `main` moved in the meantime the push is refused and nothing is released);
3. builds on Windows, in steps of its own so a failed log says which one broke: the minimal static FFmpeg libraries the audio decoder links (cached; there is no ffmpeg program, audio is decoded in-app), the pinned yt-dlp (SHA-256 checked against the value pinned in the repo), the Rust bridge (`cargo build -p pyxee-bridge --release`, with `PYXEE_UPDATE_PUBLIC_KEY` and `PYXEE_DISCORD_CLIENT_ID` compiled in) and the Flutter app (`flutter build windows --release`). Then `cargo xtask installer` and `cargo xtask portable` (with `PYXEE_BUILD_READY=1`, so they only gather, package and sign) make the two downloads. The workflow **unpacks the zip and starts it, installs the installer silently and starts that, checks that no ffmpeg program or libav DLL slipped in and that the licence texts are installed, and uninstalls it again to see that the uninstaller removes what the installer wrote**;
4. writes the release notes (below), creates the release as a draft, uploads the installer, the portable zip, `SHA256SUMS.txt`, the third-party notices and the FFmpeg LGPL text, then publishes it. A signed build-provenance attestation is added when the repository allows it.

In a repository with no `v*.*.*` tag yet, the first release publishes the version already in the files as it is. The version the files carry now is shown by `python scripts/bump_version.py --current`.

Every version, pre-releases (`-rc1`...) included, gets the NSIS installer (`Pyxee_x.y.z_x64-setup.exe`, a plain NSIS script in `packaging/windows/installer.nsi`) and the portable zip. There is no MSI.

## Building the same thing locally

On Windows, with Rust, the Flutter SDK, MSYS2 (for FFmpeg), the Visual Studio C++ tools and NSIS (`makensis`; set `PYXEE_MAKENSIS` if it is not in its default folder): `cargo xtask installer` and `cargo xtask portable` build everything they need (`cargo xtask ffmpeg`, the bridge, the Flutter app) and write `target\installer\Pyxee_<version>_x64-setup.exe` and `target\portable\Pyxee_<version>_x64-portable.zip`. `cargo xtask stage` stops after gathering the folder, and `cargo xtask smoke [folder]` starts a staged or installed copy for a few seconds (`PYXEE_SMOKE_SECONDS`) and runs its `yt-dlp.exe --version`. `scripts\check-windows.cmd -Installer` does all of that and inspects the results.

## Release notes

`scripts/release_notes.py` writes the "What's changed" section. It tries, in order:

| Layer | When it is used |
|---|---|
| **AI summary** of the real diff, in plain user-facing language | a key is configured (below), otherwise GitHub Models with the built-in token |
| Commit messages grouped into Added / Fixed / Changed / Removed (`WP4:` style labels are dropped) | no AI service answered |
| What the code shows for vague messages ("Update main.rs"): new app commands, new settings, new screens, new public items, files per area | commits that say nothing |

Nothing needs configuring: GitHub Models is free and uses the workflow's own token. For better notes add one of these in **Settings -> Secrets and variables -> Actions**:

| Name | Kind | Meaning |
|---|---|---|
| `ANTHROPIC_API_KEY` | secret | use Claude (default model `claude-sonnet-5-5`) |
| `AI_API_KEY` + `AI_BASE_URL` + `AI_MODEL` | secret + 2 variables | any OpenAI-compatible service (Gemini, Groq...) |
| `RELEASE_NOTES_MODEL` | variable | override the model for whichever service is used |

Preview the notes for any two tags on your own machine: `python scripts/release_notes.py --tag v0.2.0 --prev v0.1.0 --repo Pylxyr/Pyxee` (add `--ai` with a key in the environment).

## Dry run

**Actions -> Release -> Run workflow**, enter a version. It builds the installer and the portable zip, tests both and uploads them as a workflow artifact, and publishes nothing. The version must already be in the files (`python scripts/bump_version.py 0.2.0` first, on a branch).

## If a release fails

The run summary says what to do. Short version: a flaky step (network, runner) -> **Re-run failed jobs**, which is safe to repeat. A real problem -> fix it on `main`, `git push --delete origin vX.Y.Z`, and cut the release again.

## Repository settings it relies on

- **Settings -> Actions -> General -> Workflow permissions**: *Read and write*, or the bot cannot push the version commit and tag.
- If `main` is protected, allow the `github-actions[bot]` to push to it (or use a rules bypass for it). Make the **`ci-ok`** check the required one: it fails if any CI job failed.
- Optional: Dependabot opens weekly update PRs for GitHub Actions, Cargo and pub (`.github/dependabot.yml`); action versions are pinned to commit SHAs, so they only change through those PRs.

## Versions by hand

`python scripts/bump_version.py 0.2.0` sets every file (`Cargo.toml`, `Cargo.lock` and `ui_flutter/pubspec.yaml`); `--check v0.2.0` verifies them; `--next minor` prints the next version. CI runs the check on every push so the files cannot drift apart.

## Portable build
`cargo xtask portable` (Windows) writes `target/portable/Pyxee_<version>_x64-portable.zip`. The zip holds one folder: the app exactly as the installer installs it (`Pyxee.exe`, `yt-dlp.exe`, `pyxee_bridge.dll`, `flutter_windows.dll`, the plugin DLLs, the Visual C++ runtime, Flutter's `data\` folder and the licence texts) plus `README.txt` and `portable.txt`. `portable.txt` next to the program is what turns portable mode on: the library, settings and logs live in `data\` beside `Pyxee.exe` (nothing in the user's profile, no Roaming-to-Local move); deleting `portable.txt` gives the normal per-user folder. That `data\` folder is also where Flutter keeps `app.so`, `icudtl.dat` and `flutter_assets`, and the two never use the same names, so the zip carries Flutter's files and never the user's (`library.db`, `shell.json`, `session.json`, `ui_prefs.json`, `.env`, `logs\`, `tools\`). It never contains an ffmpeg program or a libav DLL (libav is linked in statically); the packer refuses to finish if any of these slips in, and `scripts\check-windows.cmd -Installer` opens the finished zip again to prove it. It needs nothing installed: the window is Flutter's own and the Visual C++ runtime is in the folder.

The release workflow builds the zip, unpacks it, runs `cargo xtask smoke` on it (`yt-dlp.exe --version`, then Pyxee started for ten seconds) and attaches it to the release; it is covered by `SHA256SUMS.txt` like the installer. A portable copy never runs an installer: Settings > Updates tells it to download the new zip and unpack it over the folder (the user's files are not in the zip, so they stay). yt-dlp still updates in place, into `data\tools`.

## Notices
`licenses/` holds the licence texts and notices that ship with the app (the FFmpeg LGPL text and notice, the yt-dlp licences); the release uploads them next to the installers. They are plain committed files, and `cargo xtask stage` refuses to continue if one is missing. Nothing checks their content, so update them by hand when the FFmpeg or yt-dlp version changes. `THIRD_PARTY.txt` names the Rust crates and the Flutter and Dart packages by pointing at `Cargo.lock` and `ui_flutter/pubspec.lock`; the app's window also carries the Dart packages' own licence texts inside it (Flutter collects them when it builds).

## Bumping the bundled yt-dlp
Installers and the portable zip bundle one pinned yt-dlp release, so two builds of a tag contain the same `yt-dlp.exe`. The pin is `YTDLP_VERSION` and `YTDLP_SHA256` in `xtask/src/main.rs`; `cargo xtask tools` downloads that exact release and stops if the bytes do not match. The in-app updater is separate and still pulls the latest release at run time, which is what keeps users working when YouTube changes.

To bump it:
1. Pick the release at https://github.com/yt-dlp/yt-dlp/releases and read the `yt-dlp.exe` line of its `SHA2-256SUMS`.
2. Set `YTDLP_VERSION` and `YTDLP_SHA256` in `xtask/src/main.rs`.
3. Optionally refresh `licenses/yt-dlp-THIRD_PARTY_LICENSES.txt` from the repository at that tag, and the yt-dlp version and source URL in `licenses/THIRD_PARTY.txt`. Nothing checks them.
4. Run `cargo test -p xtask` and `python -m unittest discover -s scripts/tests`; the release workflow's smoke tests then run the new exe with the app's real format selector.

## Code signing
Signing is wired in and **off until a certificate exists**. Nothing is faked: without a certificate the build is unsigned and says so (and the release notes say "not code-signed"). It is configured with environment variables, so no secret is ever in the repository, in the installer script or on a printed command line:

| Variable | Meaning |
|---|---|
| `PYXEE_SIGN_CERT_FILE` + `PYXEE_SIGN_CERT_PASSWORD` | a `.pfx` code-signing certificate and its password |
| `PYXEE_SIGN_THUMBPRINT` | or the SHA-1 thumbprint of a certificate already in the Windows certificate store (hardware token, EV) |
| `PYXEE_SIGN_TIMESTAMP_URL` | RFC 3161 timestamp server (default `http://timestamp.digicert.com`), so signatures outlive the certificate |
| `PYXEE_SIGNTOOL` | path to `signtool.exe` (default: newest Windows SDK, then PATH) |
| `PYXEE_REQUIRE_SIGNING=1` | a build without a certificate is an error |

With a certificate configured, `cargo xtask stage` signs `Pyxee.exe` (so the same signed program is in the installer and in the zip; `yt-dlp.exe` is someone else's program and is left as published, and the DLLs are not signed), the installer script signs the uninstaller it generates (`!uninstfinalize` calling `cargo xtask sign-file`), and `cargo xtask installer` signs the installer last. Each signed file is then checked with `Get-AuthenticodeSignature` and the build fails unless it is `Valid`. To sign releases from GitHub Actions add the repository secrets `WINDOWS_CERT_PFX_BASE64` (the `.pfx`, base64) and `WINDOWS_CERT_PASSWORD`; the workflow then sets the variables above, requires signing, and the release notes drop the "not code-signed" line. **Blocked on the operator**: a code-signing certificate (an OV or EV certificate from a CA, or Azure Trusted Signing through `PYXEE_SIGNTOOL`). Also still to confirm on a Windows machine with a certificate: that NSIS's `!uninstfinalize` signs the uninstaller (the installer script is written for it, but only a run with a certificate shows it).

**What the in-app updater does with signing.** The installer is always checked against `SHA256SUMS.txt` from the same release. In addition, when the running Pyxee has a valid Authenticode signature, the downloaded installer must also be validly signed with the same signer subject (the certificate's distinguished name, compared ignoring case and spacing) before it is run; otherwise it is deleted and the update is refused. A running copy that is unsigned keeps the checksum-only check, so people on today's unsigned releases can still update. The consequence for you: once releases are signed, keep the same certificate subject across renewals, or users on the old signed version will be refused the next installer. (`pyxee_shell::update::check_publisher`; signatures are read with Windows PowerShell's `Get-AuthenticodeSignature`.)

## Discord Rich Presence ID
Releases are built with the Discord application ID baked in, so users set nothing. The ID ships inside the exe, so it cannot be truly secret, but it is kept out of logs, the UI and the docs: it is a repository **secret** (Settings > Secrets and variables > Actions > **Secrets** > `PYXEE_DISCORD_CLIENT_ID`), so Actions masks it in public logs. The release workflow passes it to the build; unset or empty builds with presence off, as do forks. Users cannot override it at run time. Never store the application's client secret or any bot token anywhere; Pyxee does not use them.

Last.fm scrobbling is not in the Flutter app yet (see `docs/migration.md`), so the release workflows no longer pass the Last.fm key and secret. When it comes back, they are three lines to add again, in `release.yml` (the `workflow_call` secrets and the build job's `env`) and in `cut-release.yml`.

## In-app updates
Settings > Updates asks `api.github.com/repos/Pylxyr/Pyxee/releases` for the newest release (drafts never count; pre-releases count only when the running build is one or when Settings > Updates > "Include pre-releases" is on, which is `checkPrereleases` in `shell.json`, off by default). "Download and install" fetches the `-setup.exe`, checks it against the release's `SHA256SUMS.txt` (written by the workflow above), starts it and quits Pyxee. The installer is kept in `%TEMP%\pyxee-update` while it runs and removed at the next start. A copy of Pyxee that is signed also refuses to update when it cannot read its own signature (Windows could not be asked), since it could not then compare the publisher; the release page has the installer. The checksum list is published with the installer in the same release, so it proves the download is intact, not that the release itself is trustworthy: that is what the publisher check (once signing is on) and the build attestation are for. A release without an installer and checksum list is reported with its page instead. The installers are unsigned until a certificate is configured (see Code signing), so SmartScreen may still ask. The decisions are in `pyxee_shell::update` (unit tested); the network and process calls are in `crates/bridge/src/updater.rs`.

yt-dlp is updated separately: its published `SHA2-256SUMS` is compared with the copy in use, a newer one is downloaded, verified, test-run (`--version`) and only then renamed over `<data folder>\tools\yt-dlp.exe`, which `YtDlpSource` prefers over the installed copy (so no restart). If any step fails the working copy is untouched; no backup copy is kept. `PYXEE_YTDLP` pins a copy and disables the updater. A Linux or macOS build is deliberately not part of this.


## Signing the update list

The in-app updater downloads the installer and `SHA256SUMS.txt` from the same release, so the checksum alone only proves the download is intact, not who published it. To close that gap the release also carries `SHA256SUMS.txt.sig`, an Ed25519 signature of that exact file, and a program built with the public key installs nothing whose list does not verify.

One-time setup:

1. `cargo xtask update-keygen` prints a seed and a public key. Keep the seed secret.
2. Store the seed as the repository **secret** `PYXEE_UPDATE_SIGNING_SEED` and the public key as the repository **variable** `PYXEE_UPDATE_PUBLIC_KEY`.
3. The next release is built with the key (the `build` job reads the variable) and signs its list (the `release` job reads the secret). A release that sets the variable but not the secret fails instead of publishing something users could not update to.

Rules: the first release that carries the key cannot be installed by older programs through a signed check (they have none), and every release after it must be signed, so keep the seed safe. Losing it means users must install a new program by hand once. Builds without the variable (local builds) keep the checksum-only check and log a warning. This is independent of the Windows code-signing certificate, which still protects the installer on the machine.
