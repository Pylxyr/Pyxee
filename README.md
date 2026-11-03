# Pyxee

A music player for Windows 10 (22H2), Windows 11 and Linux. Search for songs, queue them up, keep the ones you love, and
let them play gapless. Pyxee plays music found online.

Pyxee is a hobby project and is not affiliated with YouTube or Google. It finds music on YouTube using the open-source
tool yt-dlp.

## Install

**Windows**
1. Go to the [Releases page](https://github.com/Pylxyr/Pyxee/releases) and download `Pyxee_x.y.z_x64-setup.exe` from the newest release.
2. Run it. It installs everything Pyxee needs, and downloads the WebView2 runtime if your PC does not have it.

Windows SmartScreen may warn you the first time because the installer is not code-signed yet. Choose **More info**, then **Run anyway**.

Prefer no installer? Each release also has a portable zip. Unzip it anywhere and run `Pyxee.exe`; everything it saves
stays in a `data` folder next to it.

**Linux** (Arch, CachyOS and other distributions): see [Pyxee on Linux](docs/linux.md).

## What you can do
* **Search** for songs, videos, artists and playlists, and add them to your queue.
* **Home** brings back what you were just playing and what you play most.
* **Library** keeps your likes, playlists and listening history.
* **Lyrics**, a **sleep timer**, and playback controls from the system tray.
* **Themes**: Ink (the default), Frost, OLED black and more. Open **Settings** (the gear icon, top right), then **Appearance**.
  **Match system** switches theme with your system's light/dark mode, **Customize...** lets you edit a theme's colours,
  and **Export** / **Import a theme** let you share themes.
* **Discord**: show what you are listening to. Turn it on or off in **Settings**.

## Updates
On Windows, open **Settings** -> **Updates**. It checks for a newer Pyxee and installs it for you (a portable copy
downloads the new zip instead). On Linux, Pyxee is updated by your package manager (see [Pyxee on Linux](docs/linux.md)).
On both, **Updates** also keeps the part that finds your music (yt-dlp) up to date, which fixes most "it suddenly
stopped finding songs" problems.

## If something goes wrong
* **Songs will not load, or you see "too many requests" or "sign in to confirm you're not a bot".** First try
  **Settings** -> **Updates** and update. If it keeps happening, signing in helps a lot: open **Settings** ->
  **Advanced**, fill in **Cookie file** (an exported `cookies.txt`) or **Cookies from a browser**, press **Save** and
  restart Pyxee. An exported file is more reliable than reading the browser's cookies. How to get the file is in
  [Advanced settings](docs/advanced.md).
  **Be careful with cookies:** signing in this way can get the Google account restricted or blocked by YouTube. Use a
  throwaway account, not your main one. With **Cookies from a browser**, use a separate browser profile that is signed in
  only to that account (for Firefox, `firefox:profile-name`), so nothing else in your normal profile is read.
* **A song misbehaves.** Pyxee keeps a log. Open **Settings** -> **Diagnostics** -> **Open log folder**. Search
  `pyxee.log` for the song's name and send me the lines around it. For more detail, set **Log detail** to `debug` in
  **Settings** -> **Advanced**.
* **A warning about a "JavaScript runtime".** Pyxee still works. To silence it and keep every audio format available,
  install Deno or Node, then restart Pyxee. On Windows: `winget install DenoLand.Deno`. On Arch and CachyOS:
  `sudo pacman -S deno`. Pyxee finds either on your PATH.

## More
* [Advanced settings](docs/advanced.md): every setting in **Settings** -> **Advanced**, and where Pyxee keeps its files.
* [Pyxee on Linux](docs/linux.md): install, updates and what is different.
* [Building and developing Pyxee](docs/development.md).

## Licence
Pyxee is MIT licensed (see `LICENSE`). It uses FFmpeg libraries under the LGPLv2.1 and bundles yt-dlp as a separate program
(its Windows build is GPLv3+). The full notices ship with the installer and every release.

## Contact
Questions and bugs: [open an issue](https://github.com/Pylxyr/Pyxee/issues). Security problems: see [SECURITY.md](SECURITY.md).
