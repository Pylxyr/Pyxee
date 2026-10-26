# Windows checklist (things the sandbox could not run)

Run each command on its own line in PowerShell 5. Tick an item when it works; paste the log
(`%LOCALAPPDATA%\dev.pyxee.app\logs\pyxee.log`, Taipei time) for anything that does not.

## WP1 (bugs)

- [ ] `git pull` the bundle, then `cargo test -p pyxee-audio --test headers`, `cargo test -p pyxee-app engine_tests`, `npm test --prefix ui`.
- [ ] 403: play `yt:Ni5jPjvGxtw` (search "Songs", pick it). It should play. If not, send the log lines with "refused the stream URL", "resolving again", "details for".
- [ ] After a song starts from a Songs search, the artist, length and a sharp cover appear (queue, player bar, Now Playing); the seek bar works.
- [ ] A song with unknown length shows a striped "length unknown" bar and only the elapsed time.
- [ ] Art: no black bars; sharp at Now Playing size on your display scale.
- [ ] Log timestamps end in `+08:00` and match Taipei clock time.
- [ ] Gear icon (top right) opens "Settings"; Customize theme opens ON TOP of it; Now Playing open + Settings: sheet is above.
- [ ] Queue page: drag a row by its handle (mouse), arrows work, Esc cancels a drag. Now Playing > Up next: drag, "Play next", arrows. Try with shuffle on.

## WP2 (search filter)

- [ ] Fresh start: Search opens on Songs. Pick Videos, close and reopen the app: still Videos.

## WP3 (icon)

- [ ] Rebuild (`cargo xtask installer` or `cargo run -p pyxee-app`): the window, taskbar and Alt+Tab icon is the red disc; check it is crisp at 100% and at your display scale.
- [ ] Explorer: set the folder view to Extra large / Small icons on the built exe or installer; the 16 px and 256 px versions look right (16 px has the bolder stem and no centre dot).
- [ ] Top-tabs logo is the new mark, on Dark and on a light theme; the accent (seek bar, likes, focus ring) is the same red as the logo.
- [ ] If Windows still shows the old icon: it is the icon cache. Run `ie4uinit.exe -show`, or restart Explorer.

## WP4 (milestone 6)

Build and test: `cargo test -p pyxee-shell`, `cargo test -p pyxee-lyrics`, `cargo test -p pyxee-app`, `npm test --prefix ui`.

- [ ] **Lyrics**: Now Playing > Lyrics shows lines for a popular song and highlights the current one as it plays; clicking a line seeks; an obscure song says "No lyrics found". Turn the network off: "Could not get the lyrics" with Try again.
- [ ] **Sleep timer** (moon button on Now Playing, or Settings): "15 minutes" pauses at the end and the volume fades over the last 15 s (set it to 1 min to check); "End of this song" lets the song finish and then does NOT start the next (play starts it); "After 2 songs"; "+15 min"; Turn off.
- [ ] **Media keys**: play/pause, next, previous keys work with the window in the background; Windows' volume flyout and the lock screen show title, artist and cover.
- [ ] **Taskbar**: progress bar under the icon follows the song (green playing, yellow paused, red failed, pulsing while loading).
- [ ] **Tray**: icon appears (right-click: Open, Play/Pause, Previous, Next, Mini player, Quit). Tooltip shows the song. Left click opens the flyout above the icon; Esc or clicking elsewhere closes it; clicking the icon while open closes it (does not flicker open again). Try the taskbar at the bottom, at the top and at a side, and on a second monitor / at a different display scale.
- [ ] **Close to tray** (Settings > Windows): closing the window keeps the music playing and the icon in the tray; Quit really exits; with it off, closing exits.
- [ ] **Single instance**: starting Pyxee again brings the first one to the front instead of opening a second.
- [ ] **Start with Windows**: turn on, sign out and in; Pyxee starts (in the tray if close-to-tray is on). Turn off: Task Manager > Startup no longer lists it.
- [ ] **Toast** (off by default): turn on, minimise the window, let the song change: a notification with title and artist.
- [ ] **Mini player** (tray menu > Mini player): always on top, drag it anywhere, close and reopen: same place. Unplug the monitor it was on: it comes back on the main screen.
- [ ] **Sleep / resume**: put the PC to sleep during a song and wake it: playback resumes or the song retries. Switch Wi-Fi off during a song and on again: "Back online. Trying again." when it had failed.
- [ ] **Opt-in** `cargo check -p pyxee-app --features win-extras`: if it compiles, run with the feature and check the three thumbnail buttons on the taskbar preview and the jump list (right-click the taskbar icon: Play/Pause, Next, Previous, Mini player). Paste any compiler errors; they are confined to `shell/winextras.rs`.
- [ ] Flyout and mini player colours match the app theme; the Mica backdrop shows (same as the main window).

## Release pipeline (first real run)

The workflows were linted and their scripts tested, but the Windows build itself has not run on a GitHub runner yet.
After the first **Cut release**, tick these (the run log and summary show the details):

- [ ] CI is green on the Windows runner, including the informational "win-extras compiles" step (note whether it passes).
- [ ] **Installed layout works** step: the installed folder lists the app exe plus `yt-dlp.exe` (and no ffmpeg). If a name differs from what the step expects, fix the filter in `release.yml`.
- [ ] The release has `Pyxee_x.y.z_x64-setup.exe`, the `.msi` (stable versions only), `SHA256SUMS.txt` and `THIRD_PARTY_NOTICES.txt`; the checksum of the downloaded installer matches.
- [ ] Release notes read sensibly, and say which layer wrote them (AI line at the bottom of the changes, or commit-message sections).
- [ ] Install the downloaded installer on a clean user profile: SmartScreen "More info -> Run anyway" works, the app starts, a YouTube song plays.
- [ ] A pre-release (`Cut release` with version `0.0.2-rc1`) builds the NSIS installer only and is marked as a pre-release.

### FFmpeg licence (LGPL v2.1+, statically linked)

Pyxee is MIT; the FFmpeg libraries it links are LGPL v2.1 or later (not legal advice). Keep it simple:

- [ ] The release page has `THIRD_PARTY_NOTICES.txt` and `FFmpeg-LGPL-2.1.txt` next to the installers, and the FFmpeg section of the notices names the tag and commit that `scripts/build-ffmpeg.sh` builds (update `crates/app/licenses/THIRD_PARTY.txt` when you change that pin).
- [ ] The **Installed layout works** step passed (it also fails if an ffmpeg program or a libav DLL is in the installed app: libav is linked in statically).
- [ ] The source stays public, so anyone can rebuild with another FFmpeg (`cargo xtask ffmpeg` and a normal build).
