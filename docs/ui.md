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
A **dock** (the rail) on the left, the **masthead** (the search box) along the top of the page, the **page**, and the **deck** (the player island) along the
bottom; from 1440 px a **lane** (the queue) sits beside the page.
- **Wide (>= 900 px)**: the dock is 88 px: the logo, the four places (an icon over its name) and Settings at the bottom. The masthead is
  the search box, centred (640 px at most). The page fills the middle, its content in one centred column. The deck is an 80 px island,
  one row: the cover, title and like on the left; shuffle, previous, play, next, repeat; the seek line between its two times (it takes the
  room that is left); lyrics, queue, volume and Expand on the right (volume goes under 1100 px, the whole right group under 980 px). From
  1440 px the lane (22 vw, 320 to 480 px) is a panel beside the page: what plays and Up next as a scrolling list with **Open queue**.
  Before anything plays the island says so and keeps its height.
- **Narrow**: page, then the dock (a one-line player with the seek line along its top edge), then the bottom bar. The search
  box and Settings are at the top of the page.
Page margins grow at 1600 and 1920 px, and Now Playing has no width cap (a bigger cover and a wider queue column at 1280 and 1536 px).
Click the cover (deck or dock) to open Now Playing (Up next / Lyrics / Related). The same four destinations (`lib/destinations.ts`)
feed the dock and the bottom bar (`Navigation.svelte`, `layout="rail" | "bar"`).

**The window frame is the system's.** The main window has the normal title bar and borders of the desktop (and a shadow); the page draws
nothing above its shell. Mica still shows through, because the window is created transparent. The mini player and the flyout are
frameless (they have their own drag regions). There is no setting for the frame.

**F11** switches full screen (`setFullscreen` in `lib/windowControls.ts`); in full screen the window has no frame, and the page follows
the window's state through its resize events.

## The 1.0 skin

The look moved from restrained Windows 11 Fluent to something livelier (a block at the end of `app.css`; it overrides the layered rules
above it and is the place to change the look):

- Rounder (`--r-sm/md/lg` 9, 14, 22 px) and pill-shaped buttons, chips, tabs and search.
- A second colour, `--accent-2`, a step around the colour wheel from the accent (so it follows every theme and the cover's colour); the
  washes and the Now Playing page use it. `--gradient-accent` is the plain accent (buttons and play buttons are one flat colour).
- A soft pool of the cover's colour behind the shell (the canvas); in wide windows the dock, the queue panel and the player island float on
  it as separate rounded pieces with room between them, and the page has no box.
- Rows have no hairlines; the playing one is washed with the accent. Cover tiles and Tune in cards lift and tip under the pointer.
- Everything is drawn once (gradients, shadows) or reacts to the pointer and stops; nothing runs on its own. Transitions are
  transform and opacity only.

## Theming and Mica
CSS variables on `<html>`: `data-theme` (dark/light, follows the system unless overridden) and `data-backdrop`.
On Windows 11 the app asks for Mica (`window-vibrancy`); if that succeeds the backend reports `mica` and the page
background becomes transparent with translucent panels. On Windows 10 (or if Mica fails) it stays solid.
"Color from album art" makes the whole theme from the cover of the song that is playing (`paletteFromArt` in `lib/color.ts`: background, panels, lines, text and accent all take its hue, and text and accent are nudged until they read). The theme's light or dark stays, the theme list is switched off (inert) while it is on, and with no cover the chosen theme shows.

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
Space play/pause, Left/Right seek 5 s, N next, P previous, Esc close Now Playing, `/` or Ctrl+K jump to the search box.
Ignored while typing in a text field. Arrows are left to the widgets that use them (sliders, selects, tab lists, menus);
a focused button keeps Space, except a song row the mouse just clicked, where Space is play/pause (and its key release
is swallowed, so the row is not pressed again). On the seek bar the arrows move 5 s (Shift 10 s); the thumb stays where
it was put until the player's position reaches it, and the time labels show where it is held. Global media keys come with SMTC in milestone 6.

## Identity: Masthead, Page, Deck
- **Navigation** is the dock in wide windows and a bottom bar in narrow ones (see Layout); a pill behind the chosen place slides from one to the next (`.rail-indicator`: a transform only; every place is the same height). Search is the same box at the top of the
  page at every size. Library sections and search filters are `Segmented` controls (plain tabs on a hairline, accent underline).
- **Lists are tables.** `TrackList` draws the column names (#, Title, Artist, Time) and `TrackRow` the rows: place number (a play mark
  on hover, moving bars on the song that is playing), small cover, title, artist, length. The artist gets its own column when the
  *list* is wide enough (a container query), whatever the window does. Row buttons (like, menu) appear on hover or focus; a
  touch screen always shows them. Queue, Up next and playlists share `EditableRow`.
- **The song that is playing is marked in every list** (accent title, bars), not only in the queue.
- **Type**: Segoe UI Variable for text and Bahnschrift (installed with Windows 10 and 11; Segoe UI Variable Display is the fallback) for titles, in the
  Windows 11 ramp (12/16, 14/20, 18/24, 20/28, 30/36, 40/52), sentence case, regular and semibold. There are no tracked capitals. Times use tabular figures.
- **Stage** is the layout of a wide window: the window is a canvas (the theme's background with one soft pool of the playing cover's colour,
  `--canvas-tint`, set by `App.svelte`; Home sets `--page-tint` from its own top card while nothing plays) and the chrome floats on it as
  four raised pieces with big corners and no page box between them: the **dock** on the left (`Navigation layout="rail"`: the logo, Home,
  Explore, Library, Queue, each an icon over its label, and Settings at the bottom; a pill behind the chosen icon slides with one
  transform), the **command bar** (search, centred at the top; `/` focuses it), the **player island** along the bottom (one row, see below)
  and, from 1440 px, the **queue panel** on the right. The page's content is one centred column (max 1220 px) straight on the canvas. In a
  narrow window the bottom bar holds the four places, Settings sits beside Search and the mini player is a floating strip above the bar.
- **Home** leads with the song as a poster (`.hero`): a large cover and the title in display type (up to 96 px, two lines), with
  the ways in as buttons under it: Resume/Pause, Liked songs (with the count), Surprise me (a radio from one of your songs at random) and
  More. Then **Recently played** (a shelf of covers: the songs you played last, without the one in the top card; History opens
  Library > History, Play recent queues them; on a phone it is six rows) and **On repeat** (a numbered chart of what you play most and
  like, without what is already above; two columns when the page is 900 px wide or more). Nothing is listed twice on the page. The look
  chosen in Settings (blur, aurora, gradient, vinyl) still paints behind the top card, faded out on every side. **Picking a cover on Home
  replays your history** (no radio); radios are Explore's: "Tune in" (`TuneIn.svelte`, a row of `MixCard`s, one per song you come back to; the
  song playing now is left out, an artist is shown once) and its shelves.
- **Look**: the default themes are Studio (dark: violet-black, coral accent `#ff6b5e`) and Daylight (light: warm paper, vermilion
  `#a63d12`); the 0.7 palettes are still there as Ink and Frost. The accent is one plain colour on fills and buttons (the second colour
  around the wheel is for washes only). Headings are display type in weight 800 with tight tracking; section titles 24 px; covers have a
  6 px corner like a record sleeve; buttons and chips are pills.
- **The player island** is the memorable part: one row (the song with its like, the transport, a long seek line between its two times, then lyrics,
  queue, volume and expand), a cover lit by the accent (of the theme, or of the
  cover with "Color from album art"), and the same accent washing down the lane. The hero, the flyout and the mini player use the wash.
- Studio (violet-black, coral) is the default theme, Daylight (warm paper, vermilion) the default light one; Ink and Frost are the 0.7 palettes. The logo keeps its own red; the UI accent does not (a test keeps the defaults away from YouTube red).

## Design tokens
`app.css` has three tiers. (1) The seven theme colours, written by `theme.apply()` as `--t-*` (and the opaque `--solid`,
`--solid-2`, `--on-fg`). (2) Semantic tokens mixed from them: `--layer`, `--hover`, `--press`, `--selected`, `--stage-wash`,
`--focus`, plus radii (`--r-sm/md/lg` = 6/10/16), the hairline `--rule` and the cover's `--glow`, motion (`--dur-fast` 120 ms, `--dur-base` 200 ms, `--dur-slow` 360 ms, `--ease`, `--ease-out`) and layout
(`--deck-h`, `--lane-w`, `--page-pad`). (3) Component classes in `@layer components` (`.btn`, `.icon-btn`, `.segmented`,
`.trow`, `.deck`, ...) that read tier 2 only. Components use those and a few layout utilities; a new theme needs no CSS change.
Primitives: `Button`, `IconButton`, `Toggle` (a real checkbox drawn as a switch), `Segmented`, `PageHeader`, `SectionHead`,
`MenuItem`, `PlayButton`, `LikeButton`, `VolumeSlider`, `PlayingMark`. Motion answers something that happened. Pages slide past each other on a shared axis in the order of the tabs (a view transition named `page`, old page out in 160 ms, new page in on `--ease-out`), the cover on the deck or dock grows into the one in Now Playing and back (a container transform: `data-np-source` and `.stage-art` share the name `np-cover`), and where view transitions are missing the plain arrival (`.route-in`) stands in. Small things settle on springs (`--spring`, `--spring-pop`, sampled from a damped spring; `springOut` and `springPop` in `lib/motion.ts` are the same curves for script): the bar under the chosen tab, the release of a pressed button, the play and pause icon, the cover's play button, the slider thumb, Now Playing, sheets. A new song's cover dissolves into the old one's place through a view transition (`crossfadeCover`: both are the named element `cover-now`; the browser fades two snapshots on the compositor), and its glow, a layer drawn once, fades in; webviews without view transitions bring the cover up from a slightly larger size instead (opacity and transform only: no blur, no animated shadow). Only one view transition runs at a time, which is why the accent colour still glides with its registered property rather than through a transition of its own; a cover's picture fades in over the song's colour as it loads; a heart pops when it is switched; a row the person removes (or brings back with Undo) leaves and arrives with a short slide and fade (a list replaced as a whole swaps without it); the line being sung in synced lyrics comes into focus (full size and opacity) while the rest fade and shrink with distance (no blur: a blur per line would be a filtered layer each); rows and cover tiles reveal themselves as they scroll into view (see *Rows reveal on scroll* below); the accent glides to the next cover's colour (registered `--accent`) and a progress bar fills smoothly (registered `--pct`). Loading placeholders get a sheen; nothing else loops while the window is idle. Script motion goes through `lib/motion.ts`, which drops
lengths and distances to nothing when Windows asks for less motion; CSS motion does the same with `prefers-reduced-motion`.
Forced-colour modes (Windows High Contrast) get outlines where colour alone told things apart: text-field focus, the song that plays, the tab bar and slider thumbs.

## Scroll bars
One rule set in `app.css`: thin, rounded, tinted from `--fg` (so every theme and Mica work), quiet at rest, clearer when the
pointer is over a list, accent while dragged. Scroll areas that are not a form control (`ScrollPage`, the Now Playing
list, Explore) carry `data-scroll tabindex="0"` so the keyboard can scroll them. `design-contract.test.ts` guards both.

## Explore
A shelf of similar songs per seed (likes first, then recent plays, one per artist; the history itself is Home's "Recently played" and Library's History, not a third copy here), which
come from `related_get` (`MusicSource::related`, a YouTube mix). YouTube Music's own Explore page (Trending, new albums and
singles) is not shown: it was mostly this week's bulk releases. Everything loads one request after another, stops early on
offline / rate-limited / tool-missing, and is cached for the session (Refresh asks again). Mashups, AI-made songs and covers, study
loops and anything over 15 minutes are filtered out of every shelf (`isJunk` in `lib/explore.ts`; a title filter, so it
catches the plain giveaways only). Cover shelves scroll sideways and are paged by the arrows in their heading.
Logic: `lib/explore.ts` (pure), `lib/explore.svelte.ts`, `pages/Explore.svelte`; Rust: `crates/sources/src/ytmusic.rs`.

**Genres.** Above the shelves sit YouTube Music's own "Moods & genres" as chips (`GenreChips.svelte`): *For you* is the page
above, and anime, J-Pop, K-Pop (and J-Rock, city pop, Vocaloid, C-Pop) come first in front of the rest in the website's own order
(`lib/genres.ts`); the rest are behind *More genres*. Picking one swaps the page for that genre's page: YouTube Music's own
playlists (a cover plays the playlist: `PlaylistGrid.svelte`, `playlist_tracks_get`, one request for its first page of songs) and
its song shelves. Album shelves are left out of genre pages on purpose: the albums feed is whatever distributors released this week, where
bulk-made releases pile up, while the genre pages are editorial picks. Rust: `genres_get`, `genre_get`, `playlist_tracks_get` (`MusicSource::genres`, `genre`,
`playlist_tracks`; the website's `browse` endpoint with `FEmusic_moods_and_genres`, `FEmusic_moods_and_genres_category` and
`VL<playlist id>`, no yt-dlp run). Genres load once per session after the page, a genre is asked once per session (Refresh asks
again), and a failure only hides the chips or shows a notice with *Try again*.

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
Custom themes (up to 30) live in `localStorage` with the other theme settings (every `pyxee.*` entry is also copied to `ui-prefs.json` in the data folder by `lib/persist.ts`, and put back at start if the web view lost it; a new place that writes one must call `persist.touch()`, a test checks this); every built-in is tested for contrast.
The `set_window_theme` command makes the native frame follow a fixed theme (`null` follows Windows again).

## 1.0 polish

- **Interface size** (Settings -> Appearance, real window only): Automatic, 100%, 125%, 150% or 200%, applied as the webview's
  page zoom (`lib/uiscale.ts`, capability `core:webview:allow-set-webview-zoom`). Automatic only enlarges a window that is
  at least 2400 px wide at a 100% Windows display scale (a 4K monitor left unscaled), in quarter steps up to 200%.
- **Short windows** (900 px wide or more, 760 px high or less) get a slimmer top bar and deck and a smaller page title.
- **Queue lane** (1440 px and up) stays in place with a friendly empty state while nothing plays, so the page does not shift.
- `--accent-ink` is the accent used as text (pulled toward the text colour, 4.5:1 on the selected row); `--accent` stays for fills.
- A song without a title or artist shows "Untitled" / "Unknown artist" (`titleOf`, `artistOf`); titles use `dir="auto"` for right-to-left text.
- The seek line is 6 px; while a mouse or pen is over it a small bubble shows the time you would jump to (`Seek.svelte`, only pointer moves update it). Now Playing eases its cover down to 93 % while paused and back on play (one transform transition), and search shows a Top result card above the song list. Liking a song also sends a one-off ring out of the heart (`burst` in `lib/motion.ts`).
## Look options (Settings > Look)
Every choice is a data attribute on `<html>` (`data-corners`, `data-surfaces`, `data-panels`, `data-covers`, `data-hero`, `data-tune`;
`lib/look.ts` holds the choices, `lib/look.svelte.ts` saves them in `pyxee.look` and `App.svelte` applies them). The default of each
has no attribute, so the base rules are the default look and each option is one block at the end of `app.css`.

| Option | Choices | How |
|---|---|---|
| Corners | Round, Soft, Square | `--r-sm/md/lg/xl`, `--r-art` and `--r-pill` (buttons, tabs, the bars) are redefined; everything uses the variables |
| Surfaces | Vivid, Flat | Flat sets `--gradient-accent`/`--accent-2` to the one accent and `--glow` to nothing, and drops the tilts and shadows |
| Panels | Floating, Docked | Wide windows only: the cards become one surface cut by hairlines |
| Cover shape | Rounded, Square, Circle | One rule on the cover classes (`.thumb`, `.cover-art`, `.hero-art`, `.mix-art`, ...) |
| Home top card and Now Playing | Blur, Aurora, Gradient, Vinyl, Plain | Blur is the canvas of before; the others are painted with CSS from the cover's colors (`--hero-a/b/c`). Vinyl slides a record out from behind the cover once per song (`disc-out`). Now Playing's page follows the same choice (`data-wash` on `.np-root`, softer so the text stays readable; Vinyl is its two-glow wash without the record); **Flat surfaces always give it a plain page**. Aurora, Gradient and Vinyl are one opaque layer (`components/Wash.svelte`: `.hero-wash` on the card, `.np-wash` on Now Playing); a new cover's colors fade a new layer in over the old one (opacity only), so nothing is repainted while it blends. The "Tune in" cards do the same with `.mix-bg` |
| Explore: Tune in cards | Album colors, Theme color | The cards take the cover's two colors (`--mix-a/b`), deepened until white words read (4.5:1, `cardTones`) |

A cover's colors (`lib/color.ts`, read from a 32x32 copy of the picture's centred square: YouTube's 16:9 stills have the square art in the middle and two bars of a dark colour beside it, almost half the picture, which are not the cover and once turned a grey cover brown) are found in OKLCH, where equal steps look equal. Every pixel with some chroma (the floor is lower for dark pixels) votes for its hue with the size of its chroma, so a broad area of a muted colour does not lose to a speck, and the faint warm or cool cast of a grey sleeve does not count at all. The strongest hue is the main colour, the next one at least 45 degrees away is the second, and a third hue the third; a hue needs 4 % of the picture. If the picture has too little colour altogether (the chroma of its coloured pixels, summed, under 0.008 per pixel: measured on real covers, grey sleeves with a cast are under 0.004, a black cover with a red logo is 0.011), it has no hue: its main colour is a grey and its theme is neutral (a hue taken from stray pixels turned a grey cover pink). The second colour, when no other hue exists, is the near-black beside the main one (a yellow sleeve with black type is yellow and black) or the same colour a little darker. Nothing is turned around the colour wheel: that made a yellow cover green. A yellow that is darkened is turned toward amber (`deepHue`), as at a low lightness the hue of yellow reads as olive, and `--accent-2` (the gradient's second colour, a step of +58 degrees in OKLCH) steps the other way for a yellow or gold accent. Material's own seed scoring (`@material/material-color-utilities`) was tried on the same covers and not adopted: it picks the faint pink cast of a grey cover, falls back to Google blue (`#4285F4`) for a cover without colour, and its yellow theme is olive.
| Progress bar | Smooth glide, Steps, Once a second | `seekMode` (`lib/seekmode.svelte.ts`), saved as `pyxee.seek`; see `docs/performance.md` |

The cover's colors come from `coverColors` (`lib/color.ts`: a 32 x 32 sample, the strongest hue and the strongest one of another
hue, kept per cover, several cards asking for one cover share one read) through `coverColorsOf` (`lib/covercolors.svelte.ts`). They
are read only for the looks that use them. The colors blend from the stand-in (hash of the song) or the theme's to the cover's
over 700 ms; the registered `<color>` properties are the only custom-property transitions allowed (`css-budget.test.ts`), they end by
themselves and repaint only the card while they last.

## Home at every size
The point of Home is that one glance shows the feature card and the ways back into your music, in a window that is not
maximized too. ("Tune in", Explore's, is one row that scrolls sideways: the buttons in its heading page it; `SectionHead` finds the shelf past
the one-line hint.) The page gets compact when the window is under 960 px high: a lower feature card with a smaller cover and
title, a smaller page title, shorter cards, and under 800 px the hint line is hidden. The card stacks below 560 px wide and is larger
from 1800 px. `?home` on the standalone demo's address gives it a listening history and a few likes so Home shows its full layout.

## Not verified here
Everything runs in tests (jsdom) and compiles against Tauri on Linux, but the sandbox has no Windows WebView2, so the
real window, Mica and DPI behaviour need a first run on your PC.

## WP1 notes

- **Settings** (gear: the top right) holds Appearance, and Diagnostics while logging is on (it is off by default; `LogInfoDto.enabled`). Sheets stack in the order they were opened
  (z-index 100 and up, above Now Playing at 40); the snackbar sits above them (1000).
- **Covers** go through `lib/thumb.ts`: YouTube's 4:3 stills have black bars baked in, so small covers use `mqdefault`
  (16:9, no bars), big ones `maxresdefault` then `hq720` then `mqdefault`; Google cover URLs are resized to the displayed
  size. `Thumb` centre-crops (`object-cover`) and moves to the next URL on a 404.
- **Unknown length**: the seek bar becomes a non-interactive "length unknown" bar and only the elapsed time is shown.
- **Reordering** uses pointer events on the drag handle (`lib/reorder.ts`), not HTML5 drag and drop, which a Tauri
  window on Windows can swallow. Arrow buttons stay. Up next (Now Playing) can also be dragged and has Play next.

## Motion: signature additions
The new motions share one idea: the queue is something with weight, and the accent colour is light. Each is a one-shot that
ends by itself, moves only `transform` or `opacity`, does nothing while a song plays, and stands down under
`prefers-reduced-motion`. Each has its own commit, so it can be reverted alone.

- **Queue conveyor** (`lib/flip.ts`, `use:flipList` on the queue, Up next, the lane and a playlist). When rows are reordered
  (drag, the arrows, *Play next*), or the rows below a removed one close the gap, or the next song starts and Up next moves up
  by one, the rows that stayed glide from where they were to where they are (FLIP: one Web Animations `translateY`, `--ease-out`,
  `--dur-slow`, no stagger). A MutationObserver on the list is the only trigger, so an idle list costs nothing. Rows that arrive
  or leave keep their own enter and exit, and a list replaced as a whole has no row in common, so nothing glides. At most 40
  rows animate (the ones on screen). A second move within the glide restarts it from the new layout (a small jump). Reduced
  motion, a hidden window or a webview without `animate`: the rows just move.
- **Seek-line wake** (`SeekWake.svelte`, `.seek-wake` in `app.css`, on the deck and the dock). When another song starts, a streak
  of the accent colour runs once along the seek line (700 ms, `--ease-out`) while the new cover dissolves in: the accent as
  light. It is a 4 px clipped channel with one pre-rendered gradient moved by `transform` and faded by `opacity`; it has no fill
  after it ends, so nothing is left promoted or running (`animations=0` while a song plays). It is keyed by the song, so it also
  plays for the first song after the window opens. Reduced motion and forced colours: not drawn (it is decoration only).
- **Cover light follows the music** (`.deck-art[data-paused]`, set by the deck from `player.isPlaying`). The accent glow behind the
  deck's cover is the pre-rendered layer that already existed; now its **opacity** (the only thing that changes) is lit while a
  song plays and dimmed (0.28) while paused, loading or stalled, over `--dur-slow`. A new song's cover arrives with a dim glow
  that ignites when the sound starts. The glow's `glow-in` arrival no longer keeps a fill after it ends. One transition per
  state change, nothing while a song plays. Reduced motion: the tokens are 0 ms, so it switches instantly.
- **Shuffle swap and repeat turn** (`swapOnChange`, `turnOnChange` in `lib/motion.ts`, on the two buttons in Now Playing). The icon,
  not the button, answers a change: the shuffle arrows swap places (a 380 ms sideways shuffle) and the repeat icon turns once on
  `--spring` (backwards when repeat goes off). One Web Animations `transform` animation each, ended by itself; nothing when the
  control first appears, under reduced motion, in a hidden window or without `animate`. The easing is read from the CSS token with a
  plain fallback, so a webview that rejects `linear()` still moves.
- **Snackbar time rail** (`lib/rail.ts`, `.toast-rail`). A 2 px accent rail along the foot of a toast runs down in step with the
  toast's own timer, so the Undo window can be seen. The store (`snackbar.rails`) tells it how long the snack has left; the rail
  is one `scaleX` Web Animations animation (linear, compositor only), held while the pointer is over the toast or focus is in
  it, and carried on from where it stood over the time the timer has left. The same message shown again starts it full. It
  exists only while a toast is on screen, so the idle player has none. Reduced motion and forced colours: hidden (the toast's
  text and Undo button say everything).
- **Toss to queue** (`lib/toss.ts`, `.toss-dot`, `data-toss-dest` on the Queue tab and bar item). *Play next* and *Add to queue* in
  a song's menu throw the song at the queue: a 10 px accent dot leaves the row, arcs (a sampled quadratic curve) to the Queue tab
  and the tab bumps once when it lands (460 ms flight, 300 ms bump, `transform` and `opacity` only). The dot is created for the
  flight and removed after it; nothing is left running. Not thrown under reduced motion, in a hidden window, without `animate`,
  when the queue's door is off screen or behind an overlay (`inert`); the notice still says what happened. Not drawn in forced colours.
- **Theme crossfade** (`crossfadeTheme` in `lib/motion.ts`, used by `ThemeStore.update`). Choosing a theme, following the system or
  switching "Color from album art" crossfades the whole window through the root view transition (two snapshots faded on the
  compositor, `::view-transition-group(root)`, 300 ms) instead of cutting. While it runs `data-theme-fade` on `<html>` holds the
  accent's own 600 ms glide off, so the two do not blur. The new settings are applied inside the transition, so they land a
  moment after `update()` returns. The palette that follows the cover each song and the live preview in the theme editor never
  use it: only a person's own choice does, so it cannot cancel a song's cover dissolve (one view transition at a time). Without
  view transitions (WebKitGTK) or under reduced motion the theme simply changes.
- **Scroll edge under a pinned header** (`ScrollPage.svelte`, `.page-edge`). Once a page's body has scrolled past 8 px, a soft
  14 px edge fades in under the pinned header (opacity of a pre-rendered gradient layer, `--dur-base`), so the header reads as
  lying over the content; at the top it fades out again. It is driven by one state flip when the scroll crosses the threshold,
  **not** by a CSS scroll-driven animation (`animation-timeline`): such an animation is reported as running by
  `document.getAnimations()` for as long as the page exists, which would break `animations=0` while a song plays. It works on
  WebKitGTK too. Forced colours: not drawn (the header keeps its hairline).
- **Failure nudge** (`.fail-in`, on the failure line of the deck, the dock and Now Playing). When a song fails, its message arrives with a
  short shake that settles (360 ms, `transform` and `opacity`, no fill) instead of just swapping the text, so a failure is felt
  as well as read. It runs when the message appears, once. Reduced motion: the message simply appears.
- **Rows reveal on scroll** (`reveal` in `lib/motion.ts`, `use:reveal` on `TrackRow`, `EditableRow`, the cover tiles of `CoverGrid`,
  `PlaylistGrid` and the Library, and the artist and playlist rows of a search; `[data-reveal]` in `app.css`). A row or tile waits
  invisible until it scrolls into view, however long the list is, then plays once: the row rises on `--spring` (16 px, from above
  when it scrolls in from the top) and its cover (`data-cover`, set by `Thumb` on the picture box and on the placeholder) pops in
  70 ms after it on `--spring-pop`, turning a little; the picture then fades in over it as it loads. Rows that come into view
  together (a page opening, a quick scroll) start one after another in reading order, 34 ms apart at most and all within 280 ms.
  Only transform and opacity, as Web Animations that end by themselves, with no fill after the end; the mark comes off as the
  animation starts, and the row is dropped from the one shared IntersectionObserver (which disconnects when no row is waiting), so a
  row that has played costs nothing and never replays. Replaces the fixed per-position stagger of a page's first rows. Reduced
  motion, a webview without IntersectionObserver or `animate`: nothing is marked and the rows are simply there; a hidden window or a
  setting switched on later: they appear without animation. A row that comes back through Undo keeps its own enter.
- **Search results arrive** (`route-in` on `.results` in `Search.svelte`). The results container arrives with the page; its rows
  reveal themselves as above. A filter change keeps the same container (results stay up, dimmed, while the next load). Nothing
  new in CSS; reduced motion already stills `.route-in`.
