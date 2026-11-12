# Windows checklist (things the sandbox could not run)

Run each command on its own line in PowerShell 5. Tick an item when it works. For anything that does not, start Pyxee
from a terminal with `PYXEE_LOG=debug` (see [advanced.md](advanced.md)) and paste the console output.

## Build and start

- [ ] From the repo root: `cargo xtask ffmpeg`, `cargo xtask tools`, `cargo build -p pyxee-bridge --release`, then in `ui_flutter`: `flutter pub get`, `flutter analyze`, `flutter run -d windows`. The window opens and Home fills with music.
- [ ] `cargo test --workspace` passes (it includes the engine tests in `crates/bridge`).
- [ ] The window, taskbar and Alt+Tab icon is the red disc, crisp at 100% and at your display scale. (If it is the blue Flutter logo, replace `ui_flutter/windows/runner/resources/app_icon.ico` with `assets/icons/icon.ico`. If Windows still shows an old icon it is the icon cache: run `ie4uinit.exe -show`, or restart Explorer.)
- [ ] The top-tabs logo is the new mark on Dark and on a light theme, and the accent (seek bar, likes, focus ring) is the same red.

## Playing

- [ ] A YouTube song plays (search "Songs", pick one). If it says yt-dlp was not found, see [development.md](development.md) (`PYXEE_YTDLP`, or `cargo xtask tools` and a rebuilt bridge).
- [ ] After a song starts from a Songs search, the artist, length and a sharp cover appear (queue, player bar, Now Playing); the seek bar works by click and drag.
- [ ] A song with unknown length shows a striped "length unknown" bar and only the elapsed time.
- [ ] Art: no black bars; sharp at Now Playing size on your display scale.
- [ ] Search opens on Songs. Pick Videos, close and reopen the app: still Videos.
- [ ] Queue page: drag a row by its handle (mouse), arrows work, Esc cancels a drag. Now Playing > Up next: drag, "Play next", arrows. Try with shuffle on.
- [ ] Expand button (the player deck) and the cover in the dock open Now Playing, and Esc closes it.

## Window, tray and system

- [ ] Gear icon (top right) opens Settings; Customize theme opens on top of it; with Now Playing open, the Settings sheet is above it.
- [ ] Title bar: drag, double-click to maximise, the maximise/restore icon follows, F11 goes full screen.
- [ ] **Media keys**: play/pause, next, previous work with the window in the background; Windows' volume flyout and the lock screen show title, artist and cover.
- [ ] **Tray**: the icon appears; right-click shows Open Pyxee, Play/Pause, Next, Previous, Quit.
- [ ] **Close to tray** (Settings): closing the window keeps the music playing and the icon in the tray; Quit from the tray exits at once (the window and the icon disappear immediately); with it off, closing exits.
- [ ] **Lyrics**: Now Playing > Lyrics shows lines for a popular song and highlights the current one; clicking a line seeks; an obscure song says "No lyrics found". Turn the network off: "Could not get the lyrics" with Try again.
- [ ] **Sleep timer** (moon button on Now Playing): "15 minutes" pauses at the end and the volume fades over the last 15 s (set 1 minute to check); "End of this song" lets the song finish and does not start the next.
- [ ] Mica backdrop (Settings): it shows behind the window and the colours still read well on Dark and Light.
- [ ] **Advanced settings** (Settings): the sheet lists the settings with their current values. Type a Cookie file path, Save, and `.env` in the data folder has the line; empty the field, Save, and the line is gone. Restart: the value is used.
- [ ] **Updates** (Settings): Check for updates says what it found. With a newer release: "What's new" lists the changes, Download and install shows progress, starts the installer and Pyxee closes by itself. Check yt-dlp, then Update yt-dlp, says the new version and the next search uses it.
- [ ] **Updates at start**: with a newer release out, the "What's new" sheet opens by itself a moment after start; with none (or offline) nothing appears.
- [ ] **Discord** (a build made with `PYXEE_DISCORD_CLIENT_ID`): with Discord running and the toggle on, your status shows the song and its progress and clears on pause or quit; off clears it at once.

## Release pipeline (first real run)

Not written yet for the Flutter app: see the proposal in [releasing.md](releasing.md). When a release workflow exists,
tick these on its first run:

- [ ] CI is green on the Windows runner, including the Flutter job.
- [ ] The installed folder lists the app exe, `pyxee_bridge.dll` and `yt-dlp.exe` (and no ffmpeg or libav DLL: libav is linked in statically).
- [ ] The release has the installer, `SHA256SUMS.txt` and `THIRD_PARTY_NOTICES.txt`; the checksum of the downloaded installer matches.
- [ ] Install the downloaded installer on a clean user profile: SmartScreen "More info -> Run anyway" works, the app starts, a YouTube song plays.

### FFmpeg licence (LGPL v2.1+, statically linked)

Pyxee is MIT; the FFmpeg libraries it links are LGPL v2.1 or later (not legal advice). Keep it simple:

- [ ] The release page has `THIRD_PARTY_NOTICES.txt` and `FFmpeg-LGPL-2.1.txt` next to the installers, and the FFmpeg section of the notices names the tag and commit that `scripts/build-ffmpeg.sh` builds (update `licenses/THIRD_PARTY.txt` when you change them).
- [ ] The source stays public, so anyone can rebuild with another FFmpeg (`cargo xtask ffmpeg` and a normal build).
