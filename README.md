# Pyxee

A music player for Windows 10 (22H2) and Windows 11. Search for songs, queue them up, keep the ones you love, and let
them play gapless. Pyxee plays music found online and audio files on your PC.

Pyxee is an independent hobby project. It is **not affiliated with, endorsed by or sponsored by YouTube or Google.**

**Terms of Service.** Pyxee finds and plays music from YouTube using the open-source tool yt-dlp. Using YouTube this way may
conflict with YouTube's Terms of Service. Pyxee is for personal use, and how you use it is your responsibility.

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
  **Settings** -> **Updates** and update. If it keeps happening, signing in helps a lot: open **Settings** ->
  **Advanced**, fill in **Cookie file** (an exported `cookies.txt`) or **Cookies from a browser**, press **Save** and
  restart Pyxee. Chrome-style browsers often lock their cookies on Windows, so an exported file is more reliable than
  reading them from the browser. How to get the file is in [Advanced settings](#advanced-settings).
  **Be careful with cookies:** signing in this way can get the Google account restricted or blocked by YouTube. Use a
  throwaway account, not your main one. With **Cookies from a browser**, use a separate browser profile that is signed in
  only to that account (for Firefox, `firefox:profile-name`), so nothing else in your normal profile is read.
* **A song misbehaves.** Pyxee keeps a log. Open **Settings** -> **Diagnostics** -> **Open log folder**, or look at
  `%LOCALAPPDATA%\dev.pyxee.app\logs\pyxee.log`. Search it for the song's name and send me the lines around it. For more
  detail, set **Log detail** to `debug` in **Settings** -> **Advanced**.
* **A warning about a "JavaScript runtime".** Pyxee still works. To silence it and keep every audio format available,
  install Deno (`winget install DenoLand.Deno`) or Node, then restart Pyxee. Pyxee finds either on your PATH.

## Advanced settings
**Settings** -> **Advanced** lists the environment variables Pyxee understands. Type a value, press **Save** and restart
Pyxee. The values are kept in a file called `.env` in Pyxee's data folder (`%LOCALAPPDATA%\dev.pyxee.app\.env`, or
`data\.env` next to `Pyxee.exe` for a portable copy), one `NAME="value"` per line, so you can also edit it by hand. A
variable you set in Windows itself wins over the file. An empty field means the default.

| Variable | What it is | How to fill it |
|---|---|---|
| `PYXEE_COOKIES` | Signs yt-dlp in to your account, so YouTube is less likely to say "sign in to confirm you're not a bot" or "too many requests". | The full path to a `cookies.txt` file (Netscape format). Sign in to YouTube in your browser and export its cookies with a cookies.txt extension (for example "Get cookies.txt LOCALLY" for Chrome, "cookies.txt" for Firefox), or run `yt-dlp --cookies-from-browser firefox --cookies cookies.txt`, which saves the cookies of every site. The file holds your login, so keep it private. See yt-dlp's [How do I pass cookies to yt-dlp?](https://github.com/yt-dlp/yt-dlp/wiki/FAQ#how-do-i-pass-cookies-to-yt-dlp) |
| `PYXEE_COOKIES_FROM_BROWSER` | The same sign-in without a file: yt-dlp reads the cookies straight from a browser on this PC. Ignored when the cookie file works. | A browser name such as `firefox`, optionally with a profile after a colon (`firefox:default-release`). Chrome, Edge and Brave often lock their cookies on Windows; Firefox or a cookie file is more reliable. Use a separate profile signed in to a throwaway account (see the warning under "If something goes wrong"). |
| `PYXEE_YTDLP` | Your own yt-dlp instead of the one Pyxee installs and keeps up to date. While it is set, Pyxee does not update yt-dlp. | The full path to `yt-dlp.exe`. Leave it empty to use the built-in one. |
| `PYXEE_JS_RUNTIME` | The JavaScript runtime Pyxee tells yt-dlp to use. By default Pyxee uses Deno or, if there is none, Node, whichever it finds on your PATH. | `deno`, `node`, `quickjs` or `bun`, optionally with a path after a colon (`node:C:\Tools\node.exe`), or `none` to leave it to yt-dlp. Leave it empty for the default. |
| `PYXEE_YTMUSIC_DIRECT` | Whether songs search and the Explore page ask YouTube Music directly (on by default). See the note below. | `0` turns it off: songs are then searched with yt-dlp only and there is no Explore page. Leave it empty or `1` to keep it on. |
| `PYXEE_LOG` | How much Pyxee writes to its log file. | `error`, `warn`, `info` (the default), `debug` or `trace`. Use `debug` when you report a problem with a song. |

**About asking YouTube Music directly.** With `PYXEE_YTMUSIC_DIRECT` on, Pyxee sends the same requests the YouTube Music website sends for its Songs tab and Explore page. That is an undocumented interface, not an official API, and using it may conflict with YouTube's Terms of Service. It can also stop working whenever YouTube changes it. Set `PYXEE_YTMUSIC_DIRECT=0` to use yt-dlp only.

Variables used only for building and testing Pyxee are in [docs/development.md](docs/development.md).

## For developers
Building from source, tests and releases: see [docs/development.md](docs/development.md).

## Licences
Pyxee is MIT licensed (see `LICENSE`). It uses FFmpeg libraries under the LGPLv2.1 and bundles yt-dlp as a separate program;
yt-dlp's Windows build is a GPLv3+ combined work. The full notices are included in the installer and attached to every
release, together with the material for relinking FFmpeg. Pyxee is for personal use.

## Takedown requests and contact
If you represent a rights holder, YouTube or Google and want something in Pyxee changed or removed, or you have any other
concern, please [open an issue](https://github.com/Pylxyr/Pyxee/issues). Security problems: see [SECURITY.md](SECURITY.md).
