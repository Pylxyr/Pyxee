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
Three parts, two layers, as Windows 11 stacks them: a **rail** and a **stage** on the base layer (the page background, or Mica) and
the **page** between them as the content layer, with a rounded top-left corner.
- **Wide (>= 900 px)**: rail on the left (icons only up to 1280 px, then with names; wordmark on top, Settings at the foot),
  page in the middle (search box on top, then the view), stage on the right (cover, title, seek, transport, like, volume,
  a preview of Up next with **Open queue**). Before anything plays the stage says so; it never moves when the first song starts.
- **Narrow**: page, then the dock (a one-line player with the seek line along its top edge), then the bottom bar. The search
  box and Settings are at the top of the page.
Click the cover or the dock to open Now Playing (Up next / Lyrics / Related). The same four destinations (`lib/destinations.ts`)
feed the rail and the bottom bar (`Navigation.svelte`, `layout="rail" | "bar"`).

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
`ScrollPage`: header pinned, body scrolls. Home and Explore use it too. Layout tests assert which element scrolls.

## No native dialogs
Menus are an in-page `Sheet` (focus trap, Escape, restores focus); messages are `Snackbar`s with Undo.

## Accessibility
Real buttons, `aria-label`s on icon buttons, `aria-pressed` toggles, `role=tab`/`tabpanel`, a real `<input type=range>` for
seeking (arrow keys work, `aria-valuetext` reads "1:23 of 3:42"), visible focus ring, reduced motion respected everywhere
(skeletons, the playing bars, sheets and Now Playing), controls at least 32 px.

## Keyboard (in-app)
Space play/pause, Left/Right seek 5 s, N next, P previous, Esc close Now Playing. Ignored while typing in a text field;
Space and arrows are left to focused buttons and sliders. Global media keys come with SMTC in milestone 6.

## Identity: Rail, Page, Stage
- **Navigation** is a rail (see Layout), with a small accent bar beside the chosen place. Search is the same box at the top of the
  page at every size. Library sections and search filters are `Segmented` controls (a raised pill, accent underline).
- **Lists are tables.** `TrackList` draws the column names (#, Title, Artist, Time) and `TrackRow` the rows: place number (a play mark
  on hover, moving bars on the song that is playing), small cover, title, artist, length. The artist gets its own column when the
  *list* is wide enough (a container query), whatever the window does. Row buttons (like, menu) appear on hover or focus; a
  touch screen always shows them. Queue, Up next and playlists share `EditableRow`.
- **The song that is playing is marked in every list** (accent title, bars), not only in the queue.
- **Type** is one family, Segoe UI Variable, in the Windows 11 ramp (12/16, 14/20, 18/24, 20/28, 28/36, 40/52), sentence case,
  regular and semibold. There are no tracked capitals. Times use tabular figures.
- **Home** opens with the *hero* (what is playing, or the last song, with Resume), then "On rotation" (the table) and
  "Recently played" as covers with the text under them (`CoverGrid`; Explore's "Jump back in" is the same).
- **The stage** is the memorable part: a large cover, and the accent (of the theme, or of the cover with "Color from album art")
  washing down from the top. The hero, the flyout and the mini player use the same wash.
- Ink (blue-black with amber) is the default theme. The logo keeps its own red; the UI accent does not.

## Design tokens
`app.css` has three tiers. (1) The seven theme colours, written by `theme.apply()` as `--t-*` (and the opaque `--solid`,
`--solid-2`, `--on-fg`). (2) Semantic tokens mixed from them: `--layer`, `--hover`, `--press`, `--selected`, `--stage-wash`,
`--focus`, plus radii (`--r-sm/md/lg` = 4/8/12), motion (`--dur-fast` 120 ms, `--dur-base` 200 ms, `--ease`) and layout
(`--rail-w`, `--stage-w`, `--page-pad`). (3) Component classes in `@layer components` (`.btn`, `.icon-btn`, `.segmented`,
`.trow`, `.stage`, ...) that read tier 2 only. Components use those and a few layout utilities; a new theme needs no CSS change.
Primitives: `Button`, `IconButton`, `Toggle` (a real checkbox drawn as a switch), `Segmented`, `PageHeader`, `SectionHead`,
`MenuItem`, `PlayButton`, `LikeButton`, `VolumeSlider`, `PlayingMark`. Script motion goes through `lib/motion.ts`, which drops
lengths and distances to nothing when Windows asks for less motion; CSS motion does the same with `prefers-reduced-motion`.

## Scroll bars
One rule set in `app.css`: thin, rounded, tinted from `--fg` (so every theme and Mica work), quiet at rest, clearer when the
pointer is over a list, accent while dragged. Scroll areas that are not a form control (`ScrollPage`, the Now Playing
list, Explore) carry `data-scroll tabindex="0"` so the keyboard can scroll them. `design-contract.test.ts` guards both.

## Explore
YouTube Music's own Explore page first, shelf by shelf under its own titles: songs as covers (Trending, Top songs,
New music videos, each with **Play all**) and albums and singles as covers ("New albums & singles"; Pyxee has no album page,
so a cover opens a songs search for the release). They come from the `explore_get` command (`MusicSource::explore`, one
request to the website's `browse` endpoint, no yt-dlp run); a shelf is recognised by what it holds, so the Moods and
genres buttons and podcast episodes are simply not shown. Then "Jump back in" (recent plays) and a shelf of similar songs
per seed (likes first, then recent plays, one per artist), which come from `related_get` (`MusicSource::related`, a YouTube
mix). Everything loads one request after another, stops early on offline / rate-limited / tool-missing, and is cached for
the session (Refresh asks again). If the Explore page itself fails the page says why and the personal shelves still load.
Logic: `lib/explore.ts` (pure), `lib/explore.svelte.ts`, `pages/Explore.svelte`; Rust: `crates/sources/src/ytmusic.rs`.

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

- **Settings** (gear: the foot of the rail, or the top right when narrow) holds Appearance and Diagnostics. Sheets stack in the order they were opened
  (z-index 100 and up, above Now Playing at 40); the snackbar sits above them (1000).
- **Covers** go through `lib/thumb.ts`: YouTube's 4:3 stills have black bars baked in, so small covers use `mqdefault`
  (16:9, no bars), big ones `maxresdefault` then `hq720` then `mqdefault`; Google cover URLs are resized to the displayed
  size. `Thumb` centre-crops (`object-cover`) and moves to the next URL on a 404.
- **Unknown length**: the seek bar becomes a non-interactive "length unknown" bar and only the elapsed time is shown.
- **Reordering** uses pointer events on the drag handle (`lib/reorder.ts`), not HTML5 drag and drop, which a Tauri
  window on Windows can swallow. Arrow buttons stay. Up next (Now Playing) can also be dragged and has Play next.
