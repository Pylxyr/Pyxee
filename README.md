<div align="center">

<img src="assets/logo/png/logo-1024.png" alt="Pyxee logo" width="150" />

# Pyxee

A gapless music player for Windows and Linux that plays music found online and the music on your PC (Library > On this device: add your music folders; Pyxee reads the tags of MP3, FLAC, Ogg, Opus, M4A, AAC and WAV files).

[![Latest release](https://img.shields.io/github/v/release/Pylxyr/Pyxee?include_prereleases&color=e8061f&label=release)](https://github.com/Pylxyr/Pyxee/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/Pylxyr/Pyxee/total?color=111111&label=downloads)](https://github.com/Pylxyr/Pyxee/releases)
[![Platforms](https://img.shields.io/badge/platforms-Windows%2010%20%7C%2011%20%7C%20Linux-111111)](#install)
[![CI](https://img.shields.io/github/actions/workflow/status/Pylxyr/Pyxee/ci.yml?branch=main&label=CI)](https://github.com/Pylxyr/Pyxee/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/github/license/Pylxyr/Pyxee?color=blue)](LICENSE)

[Download](https://github.com/Pylxyr/Pyxee/releases/latest) &nbsp;|&nbsp; [Features](#features) &nbsp;|&nbsp; [Install](#install) &nbsp;|&nbsp; [Getting started](#getting-started) &nbsp;|&nbsp; [Privacy](#what-leaves-your-computer) &nbsp;|&nbsp; [Updates](#updates) &nbsp;|&nbsp; [Help](#if-something-goes-wrong) &nbsp;|&nbsp; [Documentation](#documentation)

</div>

<br />

> [!NOTE]
> Pyxee is a hobby project and is not affiliated with YouTube or Google. It finds music on YouTube itself where it can, and uses the open-source tool [yt-dlp](https://github.com/yt-dlp/yt-dlp) for the rest and whenever YouTube does not answer.

## What is Pyxee?

Pyxee is a gapless music player for Windows and Linux. You search for a song, video, artist or playlist, add it to your queue, and it plays. It keeps your likes, playlists and listening history on your own computer.

**New here?** [Install](#install) it, then follow [Getting started](#getting-started). If something does not work, see [If something goes wrong](#if-something-goes-wrong).

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
| **Themes** | Ink (the default), Frost, OLED black and more. **Match system** follows your light or dark mode, **Customize...** edits a theme's colours, and **Export** and **Import a theme** let you share them. Find them under **Settings** (the gear icon, top right) -> **Appearance**. |
| **Discord** | Shows what you are listening to. Off until you turn it on in **Settings**. |

## Install

### Windows

Windows 10 (22H2) and Windows 11.

1. Open the [Releases page](https://github.com/Pylxyr/Pyxee/releases/latest) and download `Pyxee_x.y.z_x64-setup.exe` from the newest release.
2. Run it. The installer sets up everything Pyxee needs, and downloads the WebView2 runtime if your PC does not have it.

Prefer no installer? Each release also has a portable zip. Unzip it anywhere and run `Pyxee.exe`; everything it saves stays in a `data` folder next to it.

> [!TIP]
> Windows SmartScreen may warn you the first time because the installer is not code-signed yet. Choose **More info**, then **Run anyway**.

### Linux

Pyxee is built from source on your own machine, which tunes it for your CPU. There is no prebuilt Linux download.

**Arch Linux and CachyOS** (a package is built and installed with `pacman`). Open a terminal (`Ctrl+Alt+T`) and copy these lines in:

```
sudo pacman -S --needed base-devel git
git clone https://github.com/Pylxyr/Pyxee.git ~/Pyxee
cd ~/Pyxee/packaging/arch
makepkg -sic
```

- It asks for your password, and later `Proceed? [Y/n]`: type your password (nothing shows while you type) or `Y`, then Enter.
- The first build takes a while (roughly half an hour on a fast PC) and is quiet for long stretches. That is normal.
- It cleans up after itself: the build files (several gigabytes) are deleted when it finishes.
- Start Pyxee from your application menu, or run `pyxee-app`.

**To update later**, copy these lines in. They show the version you have and the version available; if the numbers match, you are already up to date:

```
cd ~/Pyxee
git restore packaging/arch/PKGBUILD
git pull
pacman -Q pyxee
grep ^pkgver packaging/arch/PKGBUILD
```

If the second number is newer, build it (your library and settings are kept):

```
cd ~/Pyxee/packaging/arch
makepkg -sic
```

**Debian, Ubuntu, Fedora and other distributions** build from source with Rust and Node.js.

The full guide covers both, step by step for beginners and with options for advanced users: **[Pyxee on Linux](docs/linux.md)**.

## Getting started

1. Start Pyxee and use the search box to find a song, video, artist or playlist.
2. Add what you find to your queue and let it play.
3. Open **Settings** (the gear icon, top right) to change how Pyxee looks and behaves.

| Where in Settings | What you find there |
|---|---|
| **Appearance** | Themes, colour taken from the album art, and the interface size. |
| **Windows** (called **System** on Linux) | Keep playing in the tray, start with your computer, song-change notifications, tray controls, **Discord** and **Last.fm**. |
| **Updates** | Check for a new version of Pyxee and update yt-dlp. |
| **Diagnostics** | Open the log folder. Only shown while logging is turned on. |

Discord and Last.fm are optional and **off until you switch them on**. Both only work in builds that include the matching application ID or key, which the official releases do. See [What leaves your computer](#what-leaves-your-computer).

Your library, queue and the updated yt-dlp are kept in Pyxee's data folder: see [Where Pyxee keeps its files](docs/data-folder.md).

## What leaves your computer

Pyxee plays music from the internet, so it has to talk to a few services. This is what it sends and why. There is nothing to sign up for, and Pyxee never signs in to YouTube.

### Always, so that music can be found and played

| Service | Information transmitted | Purpose |
|---|---|---|
| **YouTube / Google** | What you search for, requests for the audio of the songs you play, and requests for thumbnails and cover art. | Finding music, playing it, and showing artwork. |
| **lrclib.net** | The song title, artist and duration of the song whose lyrics you are viewing. | Looking up lyrics. This happens when the lyrics panel is open for the playing song. |
| **GitHub** | A request for the list of Pyxee releases, which includes the Pyxee version you run. | Checking for a newer version of Pyxee. This happens when Pyxee starts and when you open **Settings** -> **Updates**. |

Like any request on the internet, each of these also sees your IP address. yt-dlp is a separate program that Pyxee runs when it needs it, and it makes its own connections to YouTube. When you press **Check yt-dlp** or choose to install an update, Pyxee also contacts GitHub to get yt-dlp or a new Pyxee installer.

### Only if you turn them on

| Service | Information transmitted | Purpose |
|---|---|---|
| **Last.fm** | The artist, title and length of the song playing now, and of the songs you have listened to, with the time you listened. | Scrobbling to your Last.fm account. **Opt-in:** off until you switch it on, and nothing is sent while it is off. The first time, your browser opens so you can allow Pyxee on Last.fm. Pyxee never sees your Last.fm password. |
| **Discord** | The title and artist of the song playing now. | Discord Rich Presence ("Listening to Pyxee"). **Off by default** (since version 0.6.0): nothing is shared with Discord unless you turn it on. When on, Pyxee hands the song to the Discord app running on your computer, which shows it on your profile. |

### Good to know

- Search suggestions while you type are **off** in normal builds, so what you type is only sent when you run a search.
- This section covers what Pyxee itself does. What YouTube, Google, lrclib.net, GitHub, Last.fm and Discord do with the information they receive is up to them and their own policies.
- On Windows, the installer may also download Microsoft's WebView2 runtime if your PC does not have it.

## Updates

- **Windows:** open **Settings** -> **Updates**. Pyxee checks for a newer version and installs it for you (a portable copy downloads the new zip instead). It also looks for a newer version by itself when it starts, and only tells you if there is one.
- **Linux:** Pyxee does not replace itself. The two short update steps are in the [Linux install](#linux) above, with more in [Pyxee on Linux](docs/linux.md#updating). **Updates** still tells you when a newer version is out.
- **Both:** **Updates** also keeps yt-dlp, the part that finds your music when Pyxee cannot get it from YouTube directly, up to date. This fixes most "it suddenly stopped finding songs" problems.

## If something goes wrong

<details>
<summary><b>Songs will not load, or I see "too many requests" or "sign in to confirm you're not a bot"</b></summary>

<br />

First try **Settings** -> **Updates** and update.

If it keeps happening, wait a while and try again: YouTube limits how fast one network address can ask for songs, and the limit lifts by itself. Pyxee never signs in to YouTube, and it slows its own requests down on purpose so that it stays under those limits. Changing network (for example, switching off a VPN) can help too.

</details>

<details>
<summary><b>A song misbehaves</b></summary>

<br />

Pyxee does not write a log unless it is asked to (so a quiet player leaves the disk alone). A build that has logging turned on (see [`docs/development.md`](docs/development.md), `PYXEE_LOG`) writes `pyxee.log`, and **Settings** -> **Diagnostics** -> **Open log folder** opens it; that section is only there while logging is on. If Pyxee ever crashes, it leaves one small file, `last-panic.txt`, in the `logs` folder of its data folder (see [`docs/data-folder.md`](docs/data-folder.md)); attach it to your [issue](https://github.com/Pylxyr/Pyxee/issues). Otherwise, when you open an issue, include what you were playing and what happened.

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

- [Pyxee on Linux](docs/linux.md): installing, updating and troubleshooting on Linux, and what is different from Windows.
- [Where Pyxee keeps its files](docs/data-folder.md): the data folder, the log and the updated yt-dlp.
- [Security policy](SECURITY.md): how to report a security problem, and which versions get fixes.

**Building and changing Pyxee**

- [Building and developing Pyxee](docs/development.md): prerequisites, running, testing, the optional settings and the layout of the code.
- [Releasing](docs/releasing.md): how a release is cut, built and signed.
- [Windows checklist](docs/windows-checklist.md): things to check by hand on Windows.
- [Icon and logo](docs/icon.md): the mark and its colours.

**How Pyxee works inside**

- [Core](docs/core.md): the queue, state machine and heard tracker.
- [Engine, sources and search](docs/engine.md): the audio engine, music sources, search and queue.
- [Library](docs/library.md): likes, playlists and listening history.
- [User interface](docs/ui.md): the structure of the window.
- [Windows shell integration](docs/shell.md): tray, notifications, Discord Rich Presence and Last.fm.
- [Performance](docs/performance.md) and [measuring idle resource use](docs/measuring.md).

## Built with

[Rust](https://www.rust-lang.org/), [Tauri 2](https://tauri.app/), [Svelte](https://svelte.dev/), [TypeScript](https://www.typescriptlang.org/) and [Tailwind CSS](https://tailwindcss.com/).

## License

Pyxee is MIT licensed (see [`LICENSE`](LICENSE)). It uses FFmpeg libraries under the LGPLv2.1 and bundles yt-dlp as a separate program (its Windows build is GPLv3+). The full notices ship with the installer and every release.

## Contact

Questions and bugs: [open an issue](https://github.com/Pylxyr/Pyxee/issues). Security problems: see [SECURITY.md](SECURITY.md).
