<div align="center">

<img src="assets/logo/png/logo-1024.png" alt="Pyxee logo" width="150" />

# Pyxee

A gapless music player for Windows and Linux that plays music found online and the music on your PC.

[![Latest release](https://img.shields.io/github/v/release/Pylxyr/Pyxee?include_prereleases&color=e8061f&label=release)](https://github.com/Pylxyr/Pyxee/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/Pylxyr/Pyxee/total?color=111111&label=downloads)](https://github.com/Pylxyr/Pyxee/releases)
[![Platforms](https://img.shields.io/badge/platforms-Windows%2010%20%7C%2011%20%7C%20Linux-111111)](#install)
[![CI](https://img.shields.io/github/actions/workflow/status/Pylxyr/Pyxee/ci.yml?branch=main&label=CI)](https://github.com/Pylxyr/Pyxee/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/github/license/Pylxyr/Pyxee?color=blue)](LICENSE)

[Download](https://github.com/Pylxyr/Pyxee/releases/latest) &nbsp;|&nbsp; [Features](#features) &nbsp;|&nbsp; [Install](#install) &nbsp;|&nbsp; [Getting started](docs/getting-started.md) &nbsp;|&nbsp; [Privacy](docs/privacy.md) &nbsp;|&nbsp; [Help](docs/troubleshooting.md) &nbsp;|&nbsp; [Documentation](#documentation)

</div>

<br />

> [!NOTE]
> Pyxee is a hobby project and is not affiliated with YouTube or Google. It finds music on YouTube itself where it can, and uses the open-source tool [yt-dlp](https://github.com/yt-dlp/yt-dlp) for the rest and whenever YouTube does not answer.

## Features

| Feature | Description |
|---|---|
| **Search** | Find songs, videos, artists and playlists, and add them to your queue. |
| **Gapless playback** | Queue songs up and let them play, one into the next. |
| **Crossfade** | Optional: one song fades out as the next fades in (**Settings** -> **Playback**, 2 to 12 seconds). Off by default. |
| **Your own music** | Play the music on your PC: **Library** -> **On this device**, add your music folders. Pyxee reads the tags of MP3, FLAC, Ogg, Opus, M4A, AAC and WAV files. |
| **Home** | Brings back what you were just playing and what you play most. |
| **Library** | Keeps your likes, playlists and listening history on your own computer. |
| **Lyrics** | Shows lyrics for the song that is playing. |
| **Sleep timer** | Stops playback by itself when you fall asleep. |
| **Tray controls** | Playback controls from the system tray. |
| **Themes** | Ink (the default), Frost, OLED black and more. **Match system** follows your light or dark mode, **Customize...** edits a theme's colours, and **Export** and **Import a theme** let you share them. Find them under **Settings** (the gear icon, top right) -> **Appearance**. |
| **Discord** and **Last.fm** | Show or scrobble what you are listening to. Both are off until you turn them on in **Settings**. |

## Install

### Windows

Windows 10 (22H2) and Windows 11.

1. Open the [Releases page](https://github.com/Pylxyr/Pyxee/releases/latest) and download `Pyxee_x.y.z_x64-setup.exe` from the newest release.
2. Run it. The installer sets up everything Pyxee needs, and downloads the WebView2 runtime if your PC does not have it.

Prefer no installer? Each release also has a portable zip. Unzip it anywhere and run `Pyxee.exe`; everything it saves stays in a `data` folder next to it.

> [!TIP]
> Windows SmartScreen may warn you the first time because the installer is not code-signed yet. Choose **More info**, then **Run anyway**.

Pyxee updates itself from **Settings** -> **Updates**.

### Linux

Pyxee is built from source on your own machine, which tunes it for your CPU. There is no prebuilt Linux download.

**Arch Linux and CachyOS:** open a terminal (`Ctrl+Alt+T`) and copy these lines in. The first build takes roughly half an hour and cleans up after itself; then start Pyxee from your application menu, or run `pyxee-app`.

```
sudo pacman -S --needed base-devel git
git clone https://github.com/Pylxyr/Pyxee.git ~/Pyxee
cd ~/Pyxee/packaging/arch
makepkg -sic
```

**Debian, Ubuntu, Fedora and other distributions** build from source with Rust and Node.js. Pyxee does not replace itself on Linux; the update steps are in the guide.

The full guide covers installing, updating and troubleshooting, step by step for beginners and with options for advanced users: **[Pyxee on Linux](docs/linux.md)**.

## Using Pyxee

- **[Getting started](docs/getting-started.md):** searching, the queue, what is in **Settings**, and how updates work.
- **[What leaves your computer](docs/privacy.md):** what Pyxee sends to YouTube, lrclib.net, GitHub, Last.fm and Discord, and when. Pyxee never signs in to YouTube, and Discord and Last.fm are off until you turn them on.
- **[If something goes wrong](docs/troubleshooting.md):** "too many requests" or "sign in to confirm you're not a bot", a song that misbehaves, and the "JavaScript runtime" warning. Most "it stopped finding songs" problems are fixed by **Settings** -> **Updates**.
- Still stuck? [Open an issue](https://github.com/Pylxyr/Pyxee/issues).

## Documentation

**Using Pyxee**

- [Getting started](docs/getting-started.md): first steps, **Settings** and updates.
- [What leaves your computer](docs/privacy.md): the network and privacy details.
- [If something goes wrong](docs/troubleshooting.md): fixes for the common problems.
- [Pyxee on Linux](docs/linux.md): installing, updating and troubleshooting on Linux, and what is different from Windows.
- [Where Pyxee keeps its files](docs/data-folder.md): the data folder, the log and the updated yt-dlp.
- [Security policy](SECURITY.md): how to report a security problem, and which versions get fixes.

**Building and changing Pyxee**

- [Building and developing Pyxee](docs/development.md): prerequisites, running, testing, the optional settings and the layout of the code.
- [Releasing](docs/releasing.md): how a release is cut, built and signed.
- [Icon and logo](docs/icon.md): the mark and its colours.

**How Pyxee works inside**

- [Core](docs/core.md): the queue, state machine and heard tracker.
- [Engine, sources and search](docs/engine.md): the audio engine, music sources, search and queue.
- [Library](docs/library.md): likes, playlists and listening history.
- [User interface](docs/ui.md): the structure of the window.
- [Windows shell integration](docs/shell.md): tray, notifications, Discord Rich Presence and Last.fm.
- [Performance](docs/performance.md) and [measuring idle resource use](docs/measuring.md).

## License and contact

Pyxee is MIT licensed (see [`LICENSE`](LICENSE)). It uses FFmpeg libraries under the LGPLv2.1 and bundles yt-dlp as a separate program (its Windows build is GPLv3+). The full notices ship with the installer and every release.

Built with [Rust](https://www.rust-lang.org/), [Tauri 2](https://tauri.app/), [Svelte](https://svelte.dev/), [TypeScript](https://www.typescriptlang.org/) and [Tailwind CSS](https://tailwindcss.com/).

Questions and bugs: [open an issue](https://github.com/Pylxyr/Pyxee/issues). Security problems: see [SECURITY.md](SECURITY.md).
