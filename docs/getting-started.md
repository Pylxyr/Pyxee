# Getting started

1. Start Pyxee and use the search box to find a song, video, artist or playlist.
2. Add what you find to your queue and let it play.
3. Open **Settings** (the gear icon, top right) to change how Pyxee looks and behaves.

| Where in Settings | What you find there |
|---|---|
| **Appearance** | Themes, colour taken from the album art, and the interface size. |
| **Windows** (called **System** on Linux) | Keep playing in the tray, start with your computer, song-change notifications, tray controls, **Discord** and **Last.fm**. |
| **Updates** | Check for a new version of Pyxee and update yt-dlp. |
| **Diagnostics** | Open the log folder. Only shown while logging is turned on. |

**Library** -> **Offline** keeps the songs you play on this PC, so they play again without the network. It is **off until you pick a size** (from 64 MB up to 4 GB); the songs played longest ago are removed first when it is full.

Discord and Last.fm are optional and **off until you switch them on**. Both only work in builds that include the matching application ID or key, which the official releases do. See [What leaves your computer](privacy.md).

Your library, queue and the updated yt-dlp are kept in Pyxee's data folder: see [Where Pyxee keeps its files](data-folder.md).

## Updates

- **Windows:** open **Settings** -> **Updates**. Pyxee checks for a newer version and installs it for you (a portable copy downloads the new zip instead). It also looks for a newer version by itself when it starts, and only tells you if there is one.
- **Linux:** Pyxee does not replace itself. The update steps are in [Pyxee on Linux](linux.md#updating). **Updates** still tells you when a newer version is out.
- **Both:** **Updates** also keeps yt-dlp, the part that finds your music when Pyxee cannot get it from YouTube directly, up to date. This fixes most "it suddenly stopped finding songs" problems.

Back to the [README](../README.md).
