<div align="center">

<img src="assets/logo/png/logo-1024.png" alt="Pyxee logo" width="150" />

# Pyxee

**Search it. Queue it. Let it play.**

A gapless music player for Windows and Linux that plays music found online.

[![Latest release](https://img.shields.io/github/v/release/Pylxyr/Pyxee?include_prereleases&style=for-the-badge&color=e8061f&label=release)](https://github.com/Pylxyr/Pyxee/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/Pylxyr/Pyxee/total?style=for-the-badge&color=111111&label=downloads)](https://github.com/Pylxyr/Pyxee/releases)
[![Platforms](https://img.shields.io/badge/platforms-Windows%2010%20%7C%2011%20%7C%20Linux-111111?style=for-the-badge)](#-install)

[![CI](https://img.shields.io/github/actions/workflow/status/Pylxyr/Pyxee/ci.yml?branch=main&label=CI)](https://github.com/Pylxyr/Pyxee/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/github/license/Pylxyr/Pyxee?color=blue)](LICENSE)
[![Stars](https://img.shields.io/github/stars/Pylxyr/Pyxee?style=flat&logo=github)](https://github.com/Pylxyr/Pyxee/stargazers)
[![Issues](https://img.shields.io/github/issues/Pylxyr/Pyxee)](https://github.com/Pylxyr/Pyxee/issues)

[**Download**](https://github.com/Pylxyr/Pyxee/releases/latest) &nbsp;·&nbsp; [Features](#-features) &nbsp;·&nbsp; [Install](#-install) &nbsp;·&nbsp; [Updates](#-updates) &nbsp;·&nbsp; [Help](#-if-something-goes-wrong) &nbsp;·&nbsp; [Docs](#-documentation)

</div>

<br />

> [!NOTE]
> Pyxee is a hobby project and is not affiliated with YouTube or Google. It finds music on YouTube using the open-source tool [yt-dlp](https://github.com/yt-dlp/yt-dlp).

## ✨ Features

| | |
|---|---|
| 🔎 **Search** | Find songs, videos, artists and playlists, and add them to your queue. |
| 🎶 **Gapless playback** | Queue songs up and let them play, one into the next. |
| 🏠 **Home** | Brings back what you were just playing and what you play most. |
| ❤️ **Library** | Keeps your likes, playlists and listening history. |
| 📝 **Lyrics** | Lyrics for the song that is playing. |
| ⏲️ **Sleep timer** | Fall asleep to your music and let it stop by itself. |
| 🎛️ **Tray controls** | Playback controls from the system tray. |
| 🎨 **Themes** | Ink (the default), Frost, OLED black and more. **Match system** follows your light/dark mode, **Customize...** edits a theme's colours, and **Export** / **Import a theme** let you share them. Find them under **Settings** (the gear icon, top right) → **Appearance**. |
| 💬 **Discord** | Show what you are listening to. Turn it on or off in **Settings**. |

## 📥 Install

<table>
<tr>
<td width="50%" valign="top">

### 🪟 Windows

1. Open the [**Releases page**](https://github.com/Pylxyr/Pyxee/releases/latest) and download `Pyxee_x.y.z_x64-setup.exe` from the newest release.
2. Run it. It installs everything Pyxee needs, and downloads the WebView2 runtime if your PC does not have it.

**Prefer no installer?** Each release also has a portable zip. Unzip it anywhere and run `Pyxee.exe`; everything it saves stays in a `data` folder next to it.

Works on Windows 10 (22H2) and Windows 11.

</td>
<td width="50%" valign="top">

### 🐧 Linux

Arch, CachyOS and other distributions.

Pyxee is built for your machine from source, and updated by your package manager.

**[Pyxee on Linux →](docs/linux.md)**

Install steps, updates, and what is different from Windows.

</td>
</tr>
</table>

> [!TIP]
> Windows SmartScreen may warn you the first time because the installer is not code-signed yet. Choose **More info**, then **Run anyway**.

## 🔄 Updates

- **Windows:** open **Settings** → **Updates**. It checks for a newer Pyxee and installs it for you (a portable copy downloads the new zip instead).
- **Linux:** Pyxee is updated by your package manager (see [Pyxee on Linux](docs/linux.md)).
- **Both:** **Updates** also keeps the part that finds your music (yt-dlp) up to date, which fixes most "it suddenly stopped finding songs" problems.

## 🛟 If something goes wrong

<details>
<summary><b>Songs will not load, or I see "too many requests" or "sign in to confirm you're not a bot"</b></summary>

<br />

First try **Settings** → **Updates** and update.

If it keeps happening, signing in helps a lot: open **Settings** → **Advanced**, fill in **Cookie file** (an exported `cookies.txt`) or **Cookies from a browser**, press **Save** and restart Pyxee. An exported file is more reliable than reading the browser's cookies. How to get the file is in [Advanced settings](docs/advanced.md).

> [!WARNING]
> Signing in this way can get the Google account restricted or blocked by YouTube. Use a throwaway account, not your main one. With **Cookies from a browser**, use a separate browser profile that is signed in only to that account (for Firefox, `firefox:profile-name`), so nothing else in your normal profile is read.

</details>

<details>
<summary><b>A song misbehaves</b></summary>

<br />

Pyxee keeps a log. Open **Settings** → **Diagnostics** → **Open log folder**. Search `pyxee.log` for the song's name and send me the lines around it. For more detail, set **Log detail** to `debug` in **Settings** → **Advanced**.

</details>

<details>
<summary><b>A warning about a "JavaScript runtime"</b></summary>

<br />

Pyxee still works. To silence it and keep every audio format available, install Deno or Node, then restart Pyxee. Pyxee finds either on your `PATH`.

| System | Command |
|---|---|
| Windows | `winget install DenoLand.Deno` |
| Arch and CachyOS | `sudo pacman -S deno` |

</details>

Still stuck? [Open an issue](https://github.com/Pylxyr/Pyxee/issues).

## 📖 Documentation

<table>
<tr>
<td width="50%" valign="top">

### ⚙️ Advanced

For using Pyxee your way.

- [**Advanced settings**](docs/advanced.md): every setting in **Settings** → **Advanced**, and where Pyxee keeps its files.
- [**Pyxee on Linux**](docs/linux.md): install, updates and what is different.

</td>
<td width="50%" valign="top">

### 🛠️ Development

For building and changing Pyxee.

- [**Building and developing Pyxee**](docs/development.md): prerequisites, running, testing and the layout of the code.
- [**Releasing**](docs/releasing.md): how a release is cut, built and signed.

</td>
</tr>
</table>

<div align="center">

<sub>Built with</sub><br />

[![Rust](https://img.shields.io/badge/Rust-111111?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri%202-24C8DB?style=flat&logo=tauri&logoColor=white)](https://tauri.app/)
[![Svelte](https://img.shields.io/badge/Svelte-FF3E00?style=flat&logo=svelte&logoColor=white)](https://svelte.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?style=flat&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Tailwind CSS](https://img.shields.io/badge/Tailwind%20CSS-06B6D4?style=flat&logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)

</div>

## 📜 Licence

Pyxee is MIT licensed (see [`LICENSE`](LICENSE)). It uses FFmpeg libraries under the LGPLv2.1 and bundles yt-dlp as a separate program (its Windows build is GPLv3+). The full notices ship with the installer and every release.

## 💬 Contact

Questions and bugs: [open an issue](https://github.com/Pylxyr/Pyxee/issues). Security problems: see [SECURITY.md](SECURITY.md).
