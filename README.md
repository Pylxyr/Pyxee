# Pyxee

A music player for Windows 10 (22H2) and Windows 11. Search for songs, queue them up, keep the ones you love, and let
them play gapless. Pyxee plays music found online and audio files on your PC.

## Install
1. Go to the [Releases page](https://github.com/Pylxyr/Pyxee/releases) and download `Pyxee_x.y.z_x64-setup.exe` from the newest release.
2. Run it. It installs everything Pyxee needs, and downloads the WebView2 runtime if your PC does not have it.

Windows SmartScreen may warn you the first time because the installer is not code-signed yet. Choose **More info**, then **Run anyway**.

Prefer no installer? Each release also has a portable zip. Unzip it anywhere and run `Pyxee.exe`; everything it saves
stays in a `data` folder next to it.

## What you can do
* **Search** for songs, videos, artists and playlists, and add them to your queue.
* **Home** brings back what you were just playing and what you play most.
* **Library** keeps your likes, playlists and listening history.
* **Lyrics**, a **sleep timer**, and playback controls from the system tray.
* **Themes**: Ink (the default), Frost, OLED black and more. Open **Settings** (the gear icon, top right), then **Appearance**.
  **Match system** switches theme with Windows light/dark mode, **Customize...** lets you edit a theme's colours, and
  **Export** / **Import a theme** let you share themes.
* **Discord**: show what you are listening to. Turn it on or off in **Settings**, under **Windows**.

## Updates
Open **Settings** -> **Updates**. It checks for a newer Pyxee and installs it for you (a portable copy downloads the new zip instead). It also keeps the part that finds
your music up to date, which fixes most "it suddenly stopped finding songs" problems.

## If something goes wrong
* **Songs will not load, or you see "too many requests" or "sign in to confirm you're not a bot".** First try
  **Settings** -> **Updates** and update. If it keeps happening, signing in helps a lot. Export your cookies to a
  `cookies.txt` file from a browser where you are signed in, then set the environment variable `PYXEE_COOKIES` to that
  file's full path. (To set it: search Windows for "Edit environment variables for your account", add a new variable,
  then restart Pyxee.) Setting `PYXEE_COOKIES_FROM_BROWSER` to `firefox` works too, but Chrome-style browsers often lock
  their cookies on Windows, so an exported file is more reliable.
* **A song misbehaves.** Pyxee keeps a log. Open **Settings** -> **Diagnostics** -> **Open log folder**, or look at
  `%LOCALAPPDATA%\dev.pyxee.app\logs\pyxee.log`. Search it for the song's name and send me the lines around it. For more
  detail, set the environment variable `PYXEE_LOG` to `debug`.
* **A warning about a "JavaScript runtime".** Pyxee still works. To silence it and keep every audio format available,
  install Deno: open a terminal and run `winget install DenoLand.Deno`, then restart Pyxee.

## For developers
Building from source, tests and releases: see [docs/development.md](docs/development.md).

## Licences
Pyxee is MIT licensed (see `LICENSE`). It uses FFmpeg libraries under the LGPLv2.1 and bundles yt-dlp (The Unlicense) as a
separate program. The full notices are included in the installer and attached to every release. Pyxee is for personal use.
