# What leaves your computer

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
| **Last.fm** | The artist, title and length of the song playing now, and of the songs you have listened to, with the time you listened. | Scrobbling to your Last.fm account. **Opt-in:** off until you switch it on, and nothing is sent while it is off. The first time, your browser opens so you can allow Pyxee on Last.fm. Pyxee never sees your Last.fm password. **Settings > Connections > Clear Last.fm data** signs out: it forgets the saved sign-in and deletes the scrobbles still waiting to be sent, and turns scrobbling off. |
| **Discord** | The title and artist of the song playing now. | Discord Rich Presence ("Listening to Pyxee"). **Off by default** (since version 0.6.0): nothing is shared with Discord unless you turn it on. When on, Pyxee hands the song to the Discord app running on your computer, which shows it on your profile. |

### Good to know

- Search suggestions while you type are **off** in normal builds, so what you type is only sent when you run a search.
- This section covers what Pyxee itself does. What YouTube, Google, lrclib.net, GitHub, Last.fm and Discord do with the information they receive is up to them and their own policies.
- Songs kept for playing offline (**Library** -> **Offline**, off by default) are stored only on your computer. Keeping them sends nothing extra: they are what Pyxee had already downloaded to play them.
- On Windows, the installer may also download Microsoft's WebView2 runtime if your PC does not have it.
- Pasting a YouTube or YouTube Music link into the search box looks up that one song (or lists the songs of that playlist) the way playing it would. Only the video's id is taken from the link: the rest of it (tracking parameters, the time to start at) is dropped and never sent, and a link is never sent as a search suggestion.
- **Settings** -> **Playback** -> **Output device** lists the audio devices your PC has. Which one you pick is saved in Pyxee's settings on your computer and goes nowhere.
- **Export** (on a playlist) and **Import playlist** (in **Library** -> **Playlists**) use a `.json` file in a place you choose in the system's own dialog, and send nothing. An exported file holds the YouTube id, title, artist and length of each song, and no songs from your own folders (so no file paths are handed out). An imported file is not trusted: it is size-limited and every entry is checked, and only plain YouTube songs are taken.

Back to the [README](../README.md). Pyxee's own files on your computer are described in [Where Pyxee keeps its files](data-folder.md).
