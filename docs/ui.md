# Milestone 3: UI shell

## Structure
`ui/src/lib` holds logic (stores, mock backend, theme, colour, shortcuts); `components/` and `pages/` are Svelte.
The UI never invents player state: `PlayerStore.state` is replaced only by what the backend pushes
(`player-state` event). Commands go out through typed `dispatch`. The shapes (`CommandDto`, `PlayerStateDto`, ...)
are generated from Rust by ts-rs, so a mismatch fails type-checking rather than at runtime.

`Backend` has two implementations: Tauri (`invoke`/`listen`) and `MockBackend` (browser dev and tests).

## What is real and what is simulated
Real: the `pyxee-core` queue and state machine, the typed Tauri commands/events, session save/restore
(`session.json` in the app data folder), the UI, theming, Mica.
Simulated: audio. `crates/app/src/sim.rs` plays the engine's role (load delay, audible, position, gapless
handover, prefetch, radio) so the real state machine drives the UI. Demo tracks only; since milestone 4 it runs only with PYXEE_SIM=1.


## Layout
Wide (>= 900 px): a top bar (wordmark, four tabs, search, settings) + content + player bar. Narrow: content + player bar + bottom navigation.
The player bar shows once a track exists; click it (or the chevron) to open Now Playing
(Up next / Lyrics / Related).

## Theming and Mica
CSS variables on `<html>`: `data-theme` (dark/light, follows the system unless overridden) and `data-backdrop`.
On Windows 11 the app asks for Mica (`window-vibrancy`); if that succeeds the backend reports `mica` and the page
background becomes transparent with translucent panels. On Windows 10 (or if Mica fails) it stays solid.
The accent is red by default; "Color from album art" derives it from the thumbnail and lightens/darkens it to stay readable.

## Mica-safe colours (fixed after the first Windows run)
Under Mica the page background (`--bg`) is deliberately transparent, so nothing that must stay readable or opaque may use it.
Three tokens are never touched by Mica: `--on-fg` (icon/text colour on a solid foreground button, the play button),
`--solid` (base of full-screen overlays such as Now Playing) and `--solid-2` (menus, sheets, the suggestion list).
`ui/src/lib/design-contract.test.ts` fails if a component paints text with the page background again, or if an overlay stops
using an opaque colour.

## Scrolling
The window never scrolls as a whole. Now Playing (header, art, title, seek bar, controls, tabs) is fixed and only the
Up next list scrolls; on narrow windows the art shrinks to leave room for the list. Queue, Library, Playlist and Search use
`ScrollPage`: header pinned, body scrolls. Home and Explore scroll themselves. Layout tests assert which element scrolls.

## No native dialogs
Menus are an in-page `Sheet` (focus trap, Escape, restores focus); messages are `Snackbar`s with Undo.

## Accessibility
Real buttons, `aria-label`s on icon buttons, `aria-pressed` toggles, `role=tab`/`tabpanel`, a real `<input type=range>` for
seeking (arrow keys work, `aria-valuetext` reads "1:23 of 3:42"), visible focus ring, reduced-motion respected for skeletons.

## Keyboard (in-app)
Space play/pause, Left/Right seek 5 s, N next, P previous, Esc close Now Playing. Ignored while typing in a text field;
Space and arrows are left to focused buttons and sliders. Global media keys come with SMTC in milestone 6.

## Identity
Pyxee is its own product, not a YouTube Music look-alike. The structure, not only the colours, differs:
- **Navigation** is a top bar: wordmark, four underlined tabs (Home, Explore, Library, Queue), search at the far end.
  There is no side rail and no pill chips; filters and Library sections are underline tabs.
- **Lists are tables.** Every song list (`TrackRow`) is one column of full-width rows: place number (a play mark on hover),
  a small square cover, title, the artist in its own column when there is room, length, like and menu. There are no two-column
  grids of big rows. Queue and playlists use the same rhythm with a drag handle.
- **Section headings** (`SectionHead`) are small tracked-out capitals with a count and a hairline, not big bold titles.
- **Home** opens with a "Pick up where you left off" panel (the last song, Resume, the liked-songs count), then "On rotation"
  (the table) and "Recently played" as a wall of square covers with the title over the picture. "Jump back in" on Explore is
  the same wall.
- **Player bar** is a console: a state label (Playing / Paused / Loading / Stopped), an accent edge while playing, square
  controls, an accent-coloured play button and the seek bar inline in the wide layout.
- Ink (blue-black with amber) is the default theme. There is no "YT Music" theme. The logo keeps its own red; the UI accent does not.

## Scroll bars
One rule set in `app.css`: thin, rounded, tinted from `--fg` (so every theme and Mica work), quiet at rest, clearer when the
pointer is over a list, accent while dragged. Scroll areas that are not a form control (`ScrollPage`, the Now Playing
list, Explore) carry `data-scroll tabindex="0"` so the keyboard can scroll them. `design-contract.test.ts` guards both.

## Explore
Moods (quick searches), "Jump back in" (recent plays) and a shelf of similar songs per seed (likes first, then recent plays,
one per artist). The shelves come from the `related_get` command (`MusicSource::related`, a YouTube mix), load one after
another because yt-dlp is paced, stop early on offline / rate-limited / tool-missing, and are cached for the session
(Refresh asks again). Logic: `lib/explore.ts` (pure), `lib/explore.svelte.ts`, `pages/Explore.svelte`.

## Now Playing > Related
Songs similar to the one being shown, from the same `related_get` command as Explore (`MusicSource::related`, 20 songs).
Rows are the usual track rows: click plays that song with the rest of the list queued after it, **Play all** plays from the
top, and the row's heart and menu give like, play next, add to queue and add to playlist. `lib/related.svelte.ts` asks only
while the tab is showing, waits 600 ms on a song first (skipping through songs asks for none of the skipped ones), asks at
most once per song per session, drops the answer for a song the user has left, and never retries a failure by itself. After
offline / rate-limited / tool-missing the other songs show the same message for 30 s instead of asking. A failure shows the
message with **Try again**; an empty answer says no related songs were found. Component: `components/RelatedPanel.svelte`.

## Themes
A theme is seven colours (`bg`, `surface`, `surface2`, `line`, `fg`, `muted`, `accent`) plus a dark/light base
(`ui/src/lib/themes.ts`). `theme.apply()` writes them onto `<html>` as `--t-*` variables; `app.css` maps them to the
tokens components use and keeps the built-in Ink and Frost (ids `dark` and `light`) values as fallbacks. Text on filled buttons and on the accent is
derived (`readableOn`), so a light dynamic accent never gets white text. Under Mica a grey theme background stays transparent
and a coloured one tints the backdrop (`--t-mica-tint`); panels are a veil of the text colour, so they suit any theme.
Custom themes (up to 30) live in `localStorage` with the other theme settings; every built-in is tested for contrast.
The `set_window_theme` command makes the native frame follow a fixed theme (`null` follows Windows again).

## Not verified here
Everything runs in tests (jsdom) and compiles against Tauri on Linux, but the sandbox has no Windows WebView2, so the
real window, Mica and DPI behaviour need a first run on your PC.

## WP1 notes

- **Settings** (gear, top right) holds Appearance and Diagnostics. Sheets stack in the order they were opened
  (z-index 100 and up, above Now Playing at 40); the snackbar sits above them (1000).
- **Covers** go through `lib/thumb.ts`: YouTube's 4:3 stills have black bars baked in, so small covers use `mqdefault`
  (16:9, no bars), big ones `maxresdefault` then `hq720` then `mqdefault`; Google cover URLs are resized to the displayed
  size. `Thumb` centre-crops (`object-cover`) and moves to the next URL on a 404.
- **Unknown length**: the seek bar becomes a non-interactive "length unknown" bar and only the elapsed time is shown.
- **Reordering** uses pointer events on the drag handle (`lib/reorder.ts`), not HTML5 drag and drop, which a Tauri
  window on Windows can swallow. Arrow buttons stay. Up next (Now Playing) can also be dragged and has Play next.
