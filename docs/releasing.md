# Releasing

## One click

1. On GitHub: **Actions -> Cut release -> Run workflow**. Leave the branch on `main`.
2. Pick **patch**, **minor** or **major** (or type an exact version such as `0.2.0` or `0.2.0-rc1`; a `-` makes it a pre-release) and run it.

That is all. The workflow then:

1. runs the full CI on `main` (UI check/tests/build, `cargo fmt`, clippy, tests, generated bindings, the release scripts' own tests). If anything fails, nothing is bumped or tagged;
2. works out the next version, writes it into every file that carries it, commits `Release X.Y.Z` and pushes the commit and the tag `vX.Y.Z` in one atomic push (if `main` moved in the meantime the push is refused and nothing is released);
3. builds on Windows: builds the minimal static FFmpeg libraries the audio decoder links (cached; there is no ffmpeg program, audio is decoded in-app), downloads the pinned yt-dlp (SHA-256 checked against the value pinned in the repo), builds the UI, builds the installer, **installs it silently, checks that no ffmpeg program or libav DLL slipped in and that the FFmpeg LGPL text is installed, and runs the bundled yt-dlp from where it landed**;
4. writes the release notes (below), creates the release as a draft, uploads the installers, `SHA256SUMS.txt`, the third-party notices and the FFmpeg LGPL text, then publishes it. A signed build-provenance attestation is added when the repository allows it.

The first release ever (no `v*.*.*` tag yet) publishes the version already in the files as it is. `0.0.1` is the current one.

Every version, pre-releases (`-rc1`...) included, gets the NSIS installer (`Pyxee_x.y.z_x64-setup.exe`) and the portable zip. There is no MSI.

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

**Actions -> Release -> Run workflow**, enter a version. It checks, builds and tests the installer and uploads it as a workflow artifact, and publishes nothing. The version must already be in the files (`python scripts/bump_version.py 0.2.0` first, on a branch).

## If a release fails

The run summary says what to do. Short version: a flaky step (network, runner) -> **Re-run failed jobs**, which is safe to repeat. A real problem -> fix it on `main`, `git push --delete origin vX.Y.Z`, and cut the release again.

## Repository settings it relies on

- **Settings -> Actions -> General -> Workflow permissions**: *Read and write*, or the bot cannot push the version commit and tag.
- If `main` is protected, allow the `github-actions[bot]` to push to it (or use a rules bypass for it). Make the **`ci-ok`** check the required one: it fails if any CI job failed.
- Optional: Dependabot opens weekly update PRs for GitHub Actions, Cargo and npm (`.github/dependabot.yml`); action versions are pinned to commit SHAs, so they only change through those PRs.

## Versions by hand

`python scripts/bump_version.py 0.2.0` sets every file (`Cargo.toml`, `Cargo.lock`, `ui/package.json`, its lockfile, `tauri.conf.json`); `--check v0.2.0` verifies them; `--next minor` prints the next version. CI runs the check on every push so the files cannot drift apart.

## Portable build
`cargo xtask portable` (Windows) builds the program without an installer and writes `target/portable/Pyxee_<version>_x64-portable.zip`. The zip holds one folder: `Pyxee.exe`, `yt-dlp.exe`, `THIRD_PARTY_NOTICES.txt`, `FFmpeg-LGPL-2.1.txt`, `GPL-3.0.txt`, `yt-dlp-THIRD_PARTY_LICENSES.txt`, `README.txt` and `portable.txt`. `portable.txt` next to the program is what turns portable mode on: the library, settings and logs live in `data\` beside `Pyxee.exe` (nothing in the user's profile, no Roaming-to-Local move); deleting `portable.txt` gives the normal per-user folder. The zip never contains a `data` folder, an ffmpeg program or a libav DLL (libav is linked in statically); the packer refuses to finish if one slips in, and `scripts\check-windows.cmd -Installer` opens the finished zip again to prove it. It needs the WebView2 runtime (part of Windows 11 and current Windows 10), which the installer would otherwise download.

The release workflow builds the zip after the installers, unpacks it, runs the bundled `yt-dlp.exe --version`, and attaches it to the release; it is covered by `SHA256SUMS.txt` like the installers. A portable copy never runs an installer: Settings > Updates tells it to download the new zip and unpack it over the folder (`data\` is not in the zip, so it stays). yt-dlp still updates in place, into `data\tools`.

## Notices
`crates/app/licenses/` holds the licence texts and notices that ship with the app (the FFmpeg LGPL text and notice, the yt-dlp licences); the release uploads them next to the installers. They are plain committed files: nothing generates or checks them, so update them by hand when the FFmpeg or yt-dlp version changes.

## Bumping the bundled yt-dlp
Installers and the portable zip bundle one pinned yt-dlp release, so two builds of a tag contain the same `yt-dlp.exe`. The pin is `YTDLP_VERSION` and `YTDLP_SHA256` in `xtask/src/main.rs`; `cargo xtask tools` downloads that exact release and stops if the bytes do not match. The in-app updater is separate and still pulls the latest release at run time, which is what keeps users working when YouTube changes.

To bump it:
1. Pick the release at https://github.com/yt-dlp/yt-dlp/releases and read the `yt-dlp.exe` line of its `SHA2-256SUMS`.
2. Set `YTDLP_VERSION` and `YTDLP_SHA256` in `xtask/src/main.rs`.
3. Optionally refresh `crates/app/licenses/yt-dlp-THIRD_PARTY_LICENSES.txt` from the repository at that tag, and the yt-dlp version and source URL in `crates/app/licenses/THIRD_PARTY.txt`. Nothing checks them.
4. Run `cargo test -p xtask` and `python -m unittest discover -s scripts/tests`; the release workflow's smoke tests then run the new exe with the app's real format selector.

## Code signing
Signing is wired in and **off until a certificate exists**. Nothing is faked: without a certificate the build is unsigned and says so (and the release notes say "not code-signed"). It is configured with environment variables, so no secret is ever in the repository, in a Tauri config or on a printed command line:

| Variable | Meaning |
|---|---|
| `PYXEE_SIGN_CERT_FILE` + `PYXEE_SIGN_CERT_PASSWORD` | a `.pfx` code-signing certificate and its password |
| `PYXEE_SIGN_THUMBPRINT` | or the SHA-1 thumbprint of a certificate already in the Windows certificate store (hardware token, EV) |
| `PYXEE_SIGN_TIMESTAMP_URL` | RFC 3161 timestamp server (default `http://timestamp.digicert.com`), so signatures outlive the certificate |
| `PYXEE_SIGNTOOL` | path to `signtool.exe` (default: newest Windows SDK, then PATH) |
| `PYXEE_REQUIRE_SIGNING=1` | a build without a certificate is an error |

With a certificate configured, `cargo xtask installer` and `cargo xtask portable` sign everything Tauri produces (the program, the uninstaller, the NSIS installer, through `bundle.windows.signCommand` calling `cargo xtask sign-file`) and `Pyxee.exe` in the zip (`yt-dlp.exe` is someone else's program and is left as published), then check each file with `Get-AuthenticodeSignature` and fail unless it is `Valid`. To sign releases from GitHub Actions add the repository secrets `WINDOWS_CERT_PFX_BASE64` (the `.pfx`, base64) and `WINDOWS_CERT_PASSWORD`; the workflow then sets the variables above, requires signing, and the release notes drop the "not code-signed" line. **Blocked on the operator**: a code-signing certificate (an OV or EV certificate from a CA, or Azure Trusted Signing through `PYXEE_SIGNTOOL`). Also still to confirm on a Windows machine with a certificate: that the installed Tauri CLI accepts the `{cmd, args}` form of `signCommand` used here.

**What the in-app updater does with signing.** The installer is always checked against `SHA256SUMS.txt` from the same release. In addition, when the running Pyxee has a valid Authenticode signature, the downloaded installer must also be validly signed with the same signer subject (the certificate's distinguished name, compared ignoring case and spacing) before it is run; otherwise it is deleted and the update is refused. A running copy that is unsigned keeps the checksum-only check, so people on today's unsigned releases can still update. The consequence for you: once releases are signed, keep the same certificate subject across renewals, or users on the old signed version will be refused the next installer. (`pyxee_shell::update::check_publisher`; signatures are read with Windows PowerShell's `Get-AuthenticodeSignature`.)

## Discord Rich Presence ID
Releases are built with the Discord application ID baked in, so users set nothing. The ID ships inside the exe, so it cannot be truly secret, but it is kept out of logs, the UI and the docs: it is a repository **secret** (Settings > Secrets and variables > Actions > **Secrets** > `PYXEE_DISCORD_CLIENT_ID`), so Actions masks it in public logs. The release workflow passes it to the build; unset or empty builds with presence off, as do forks. Users cannot override it at run time. Never store the application's client secret or any bot token anywhere; Pyxee does not use them.

Last.fm scrobbling works the same way: releases are built with the Last.fm API account's key and shared secret baked in. Both are repository **secrets** (`PYXEE_LASTFM_API_KEY` and `PYXEE_LASTFM_API_SECRET`, same place as above), passed by `cut-release.yml` to `release.yml` and on to the build. Unset or empty builds with scrobbling unavailable, as do forks. Each user signs in to their own Last.fm account in the browser; only their session key is saved, in the Windows Credential Manager (older versions kept it in `shell.json`; it is moved there the first time it is seen).

## In-app updates
Settings > Updates asks `api.github.com/repos/Pylxyr/Pyxee/releases` for the newest release (drafts never count; pre-releases count only when the running build is one). "Download and install" fetches the `-setup.exe`, checks it against the release's `SHA256SUMS.txt` (written by the workflow above), starts it and quits Pyxee. The installer is kept in `%TEMP%\pyxee-update` while it runs and removed at the next start. A copy of Pyxee that is signed also refuses to update when it cannot read its own signature (Windows could not be asked), since it could not then compare the publisher; the release page has the installer. The checksum list is published with the installer in the same release, so it proves the download is intact, not that the release itself is trustworthy: that is what the publisher check (once signing is on) and the build attestation are for. A release without an installer and checksum list is reported with its page instead. The installers are unsigned until a certificate is configured (see Code signing), so SmartScreen may still ask. The decisions are in `pyxee_shell::update` (unit tested); the network and process calls are in `crates/app/src/updater.rs`.

yt-dlp is updated separately: its published `SHA2-256SUMS` is compared with the copy in use, a newer one is downloaded, verified, test-run (`--version`) and only then renamed over `<data folder>\tools\yt-dlp.exe`, which `YtDlpSource` prefers over the installed copy (so no restart). If any step fails the working copy is untouched; no backup copy is kept. `PYXEE_YTDLP` pins a copy and disables the updater. A Linux or macOS build is deliberately not part of this.


## Signing the update list

The in-app updater downloads the installer and `SHA256SUMS.txt` from the same release, so the checksum alone only proves the download is intact, not who published it. To close that gap the release also carries `SHA256SUMS.txt.sig`, an Ed25519 signature of that exact file, and a program built with the public key installs nothing whose list does not verify.

One-time setup:

1. `cargo xtask update-keygen` prints a seed and a public key. Keep the seed secret.
2. Store the seed as the repository **secret** `PYXEE_UPDATE_SIGNING_SEED` and the public key as the repository **variable** `PYXEE_UPDATE_PUBLIC_KEY`.
3. The next release is built with the key (the `build` job reads the variable) and signs its list (the `release` job reads the secret). A release that sets the variable but not the secret fails instead of publishing something users could not update to.

Rules: the first release that carries the key cannot be installed by older programs through a signed check (they have none), and every release after it must be signed, so keep the seed safe. Losing it means users must install a new program by hand once. Builds without the variable (local builds) keep the checksum-only check and log a warning. This is independent of the Windows code-signing certificate, which still protects the installer on the machine.
