<div align="center">

<img src="assets/logo/png/logo-1024.png" alt="Pyxee logo" width="150" />

# Pyxee

**Search it. Queue it. Let it play.**

A gapless music player for Windows that plays music found online. (A Linux version is being worked on.)

[![Latest release](https://img.shields.io/github/v/release/Pylxyr/Pyxee?include_prereleases&color=e8061f&label=release)](https://github.com/Pylxyr/Pyxee/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/Pylxyr/Pyxee/total?color=111111&label=downloads)](https://github.com/Pylxyr/Pyxee/releases)
[![Platforms](https://img.shields.io/badge/platforms-Windows%2010%20%7C%2011-111111)](#install)
[![CI](https://img.shields.io/github/actions/workflow/status/Pylxyr/Pyxee/ci.yml?branch=main&label=CI)](https://github.com/Pylxyr/Pyxee/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/github/license/Pylxyr/Pyxee?color=blue)](LICENSE)

[Download](https://github.com/Pylxyr/Pyxee/releases/latest) &nbsp;|&nbsp; [Features](#features) &nbsp;|&nbsp; [Install](#install) &nbsp;|&nbsp; [Updates](#updates) &nbsp;|&nbsp; [Help](#if-something-goes-wrong) &nbsp;|&nbsp; [Documentation](#documentation)

</div>

<br />

> [!NOTE]
> Pyxee is a hobby project and is not affiliated with YouTube or Google. It finds music on YouTube using the open-source tool [yt-dlp](https://github.com/yt-dlp/yt-dlp).

## Features

| Feature | Description |
|---|---|
| **Search** | Find songs, videos, artists and playlists, and add them to your queue. |
| **Gapless playback** | Queue songs up and let them play, one into the next. |
| **Home** | Brings back what you were just playing and what you play most. |
| **Library** | Keeps your likes, playlists and listening history. |
| **Lyrics** | Shows lyrics for the song that is playing. |
| **Sleep timer** | Stops playback by itself when you fall asleep. |
| **Tray controls** | Playback controls from the system tray. |
| **Discord** | Shows what you are listening to. Turn it on or off in **Settings**. |
| **Themes** | Ink (the default), Frost, OLED black and more. **Match system** follows your light or dark mode, **Customize...** edits a theme's colours, and **Export** and **Import a theme** let you share them. Find them under **Settings** (the gear icon, top right) -> **Appearance**. |

## Install

### Windows

Windows 10 (22H2) and Windows 11.

1. Open the [Releases page](https://github.com/Pylxyr/Pyxee/releases/latest) and download `Pyxee_x.y.z_x64-setup.exe` from the newest release.
2. Run it. The installer sets up everything Pyxee needs.

Prefer no installer? Each release also has a portable zip. Unzip it anywhere and run `Pyxee.exe`; everything it saves stays in a `data` folder next to it.

> [!TIP]
> Windows SmartScreen may warn you the first time because the installer is not code-signed yet. Choose **More info**, then **Run anyway**.

### Linux

There is no Linux version to install yet: the Linux package of the earlier release was built for the old window system and
has been removed, and the new one has not been built or tested. See [Pyxee on Linux](docs/linux.md) for what works today.

## Updates

Open **Settings** -> **Updates**. Pyxee checks for a newer version, shows what is new, and installs it for you (a portable copy tells you where to download the new zip instead). It also looks once by itself when it starts and only speaks up when there is something new.

**Updates** also keeps yt-dlp, the part that finds your music, up to date. This fixes most "it suddenly stopped finding songs" problems.

## If something goes wrong

<details>
<summary><b>Songs will not load, or I see "too many requests" or "sign in to confirm you're not a bot"</b></summary>

<br />

First try **Settings** -> **Updates** and update.

If it keeps happening, signing in helps a lot. Open **Settings** -> **Advanced settings**, fill in **Cookie file** (an exported `cookies.txt`) or **Cookies from a browser**, press **Save** and restart Pyxee. An exported file is more reliable than reading the browser's cookies. How to get the file is described in [Advanced settings](docs/advanced.md).

This version has no log file. Start Pyxee from a terminal with `PYXEE_LOG` set to `debug` (in PowerShell: `$env:PYXEE_LOG="debug"` and then run `Pyxee.exe`), play the song again, and copy the lines about it from the terminal. When you [open an issue](https://github.com/Pylxyr/Pyxee/issues), include them.

</details>

<details>
<summary><b>A warning about a "JavaScript runtime"</b></summary>

<br />

Pyxee still works. To silence the warning and keep every audio format available, install Deno or Node.js, then restart Pyxee. Pyxee finds either on your `PATH`.

| System | Command |
|---|---|
| Windows | `winget install DenoLand.Deno` |
| Arch and CachyOS | `sudo pacman -S deno` |
| Other Linux | Install `nodejs` with your package manager, or get Deno from [deno.com](https://deno.com/) |

</details>

Still stuck? [Open an issue](https://github.com/Pylxyr/Pyxee/issues).

## Documentation

**Using Pyxee**

- [Pyxee on Linux](docs/linux.md): the state of the Linux version.
- [Advanced settings](docs/advanced.md): every setting in **Settings** -> **Advanced settings**, and where Pyxee keeps its files.
- [What changed from the Tauri version](docs/migration.md): features that carried over and features that are not back yet.

**Building and changing Pyxee**

- [Building and developing Pyxee](docs/development.md): prerequisites, running, testing and the layout of the code.
- [Releasing](docs/releasing.md): how a release is cut, built and signed.

## Built with

[Rust](https://www.rust-lang.org/), [Flutter](https://flutter.dev/) and [Dart](https://dart.dev/).

## Licence

Pyxee is MIT licensed (see [`LICENSE`](LICENSE)). It uses FFmpeg libraries under the LGPLv2.1 and bundles yt-dlp as a separate program (its Windows build is GPLv3+). The full notices ship with the installer and every release.

## Contact

Questions and bugs: [open an issue](https://github.com/Pylxyr/Pyxee/issues). Security problems: see [SECURITY.md](SECURITY.md).
