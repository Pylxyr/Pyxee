# Advanced settings

**Settings** -> **Advanced settings** lists the environment variables Pyxee understands. Type a value, press **Save** and
restart Pyxee. The values are kept in a file called `.env` in Pyxee's data folder, one `NAME="value"` per line, so you can
also edit it by hand. A variable you set in the system itself wins over the file. An empty field means the default.

| System | Data folder (holds `.env` and `session.json`) |
|---|---|
| Windows | `%LOCALAPPDATA%\dev.pyxee.app`, or `data\` next to `Pyxee.exe` for a portable copy |
| Linux | `~/.local/share/dev.pyxee.app` |

| Variable | What it is | How to fill it |
|---|---|---|
| `PYXEE_COOKIES` | Signs yt-dlp in to your account, so YouTube is less likely to say "sign in to confirm you're not a bot" or "too many requests". | The full path to a `cookies.txt` file (Netscape format). Sign in to YouTube in your browser and export its cookies with a cookies.txt extension (for example "Get cookies.txt LOCALLY" for Chrome, "cookies.txt" for Firefox), or run `yt-dlp --cookies-from-browser firefox --cookies cookies.txt`, which saves the cookies of every site. The file holds your login, so keep it private. See yt-dlp's [How do I pass cookies to yt-dlp?](https://github.com/yt-dlp/yt-dlp/wiki/FAQ#how-do-i-pass-cookies-to-yt-dlp) |
| `PYXEE_COOKIES_FROM_BROWSER` | The same sign-in without a file: yt-dlp reads the cookies straight from a browser on this PC. Ignored when the cookie file works. | A browser name such as `firefox`, optionally with a profile after a colon (`firefox:default-release`). Chrome, Edge and Brave often lock their cookies on Windows; Firefox or a cookie file is more reliable. On Linux, browsers that keep their cookies in the desktop keyring need that keyring unlocked. Use a separate profile signed in to a throwaway account (see the warning in the [README](../README.md#if-something-goes-wrong)). |
| `PYXEE_YTDLP` | Your own yt-dlp instead of the one Pyxee installs and keeps up to date. While it is set, Pyxee does not update yt-dlp. | The full path to `yt-dlp.exe` on Windows or `yt-dlp` on Linux (the file name has to start with `yt-dlp`). Leave it empty to use the built-in one. |
| `PYXEE_JS_RUNTIME` | The JavaScript runtime Pyxee tells yt-dlp to use. By default Pyxee uses Deno or, if there is none, Node, whichever it finds on your PATH. | `deno`, `node`, `quickjs` or `bun`, optionally with a path after a colon (`node:C:\Tools\node.exe`), or `none` to leave it to yt-dlp. Leave it empty for the default. |
| `PYXEE_YTDLP_PLAYER_CLIENT` | The YouTube client yt-dlp uses (`--extractor-args youtube:player_client=...`). By default Pyxee skips `tv_samsung`, whose stream URLs YouTube refuses with HTTP 403 as of yt-dlp 2026.10.21. | A client name such as `visionos`, or several separated by commas. `default,-name` drops one from yt-dlp's own list. `none` passes nothing and leaves it to yt-dlp. Leave it empty for Pyxee's default. |
| `PYXEE_YTMUSIC_DIRECT` | Whether songs search and the Explore page ask YouTube Music directly (on by default). See the note below. | `0` turns it off: songs are then searched with yt-dlp only and there is no Explore page. Leave it empty or `1` to keep it on. |
| `PYXEE_SUGGESTIONS` | Whether the search box asks Google for suggestions while you type (on by default). | `0` turns it off: nothing you type is sent anywhere until you run a search. Leave it empty or `1` to keep it on. |
| `PYXEE_LOG` | How much Pyxee writes to the console (standard error). There is no log file in this version. | `error`, `warn`, `info`, `debug` or `trace`; `off` writes nothing; empty means `warn`. Use `debug` when you report a problem with a song, and start Pyxee from a terminal to see it. |

**About asking YouTube Music directly.** With `PYXEE_YTMUSIC_DIRECT` on, Pyxee sends the same requests the YouTube Music website sends for its Songs tab and Explore page. That is an undocumented interface, not an official API, so it can stop working whenever YouTube changes it. Set `PYXEE_YTMUSIC_DIRECT=0` to use yt-dlp only.

Variables used only for building and testing Pyxee are in [development.md](development.md).

