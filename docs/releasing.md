# Releasing

## One click

1. On GitHub: **Actions -> Cut release -> Run workflow**. Leave the branch on `main`.
2. Pick **patch**, **minor** or **major** (or type an exact version such as `0.2.0` or `0.2.0-rc1`; a `-` makes it a pre-release) and run it.

That is all. The workflow then:

1. runs the full CI on `main` (UI check/tests/build, `cargo fmt`, clippy, tests, generated bindings, the release scripts' own tests). If anything fails, nothing is bumped or tagged;
2. works out the next version, writes it into every file that carries it, commits `Release X.Y.Z` and pushes the commit and the tag `vX.Y.Z` in one atomic push (if `main` moved in the meantime the push is refused and nothing is released);
3. builds on Windows: builds the minimal static FFmpeg libraries the audio decoder links (cached; there is no ffmpeg program, audio is decoded in-app), downloads yt-dlp (SHA-256 verified), builds the UI, builds the installer, **installs it silently, checks that no ffmpeg program or libav DLL slipped in and that the FFmpeg LGPL text is installed, and runs the bundled yt-dlp from where it landed**;
4. writes the release notes (below), creates the release as a draft, uploads the installers, `SHA256SUMS.txt`, the third-party notices and the FFmpeg LGPL text, then publishes it. The licence items to check each time are in `docs/windows-checklist.md` ("FFmpeg licence"). A signed build-provenance attestation is added when the repository allows it.

The first release ever (no `v*.*.*` tag yet) publishes the version already in the files as it is. `0.0.1` is the current one.

Stable versions get an NSIS installer (`Pyxee_x.y.z_x64-setup.exe`) and an MSI. Pre-releases (`-rc1`...) get the NSIS installer only, because Windows Installer rejects non-numeric version suffixes.

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

## Not done yet

Code signing (SmartScreen still warns), the portable build and in-app updates (milestone 7). A Linux or macOS build is deliberately not part of this.
