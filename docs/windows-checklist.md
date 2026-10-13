# Windows checklist (things the sandbox could not run)

Run each command on its own line in PowerShell 5. Tick an item when it works; paste the log
(`%APPDATA%\dev.pyxee.app\logs\pyxee.log`, Taipei time) for anything that does not.

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
- [ ] Side rail logo is the new mark, on Dark and on a light theme; the accent (seek bar, likes, focus ring) is the same red as the logo.
- [ ] If Windows still shows the old icon: it is the icon cache. Run `ie4uinit.exe -show`, or restart Explorer.
