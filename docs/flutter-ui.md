# Flutter UI (ui_flutter)

The Flutter shell recreates the Tauri/Svelte UI (`ui/`), screen for screen, on top of the Rust core through
`pyxee-bridge`. Nothing in `ui/` is used by it.

## Map: Svelte to Flutter
| Svelte (`ui/src`) | Flutter (`ui_flutter/lib`) |
|---|---|
| `app.css` tokens, `lib/themes.ts`, `lib/color.ts` | `app/theme/colors.dart` (22 built-in themes, `paletteFromArt`), `app/theme/px_theme.dart` (`PxColors`, type ramp, Mica tints) |
| `App.svelte` (masthead / deck / lane / dock / bottom bar) | `app/shell/shell.dart`, `navigation.dart`, `top_bar.dart`, `deck.dart`, `title_bar.dart` |
| `lib/nav.svelte.ts`, search and snackbar stores | `app/state/ui_state.dart` |
| `components/*` (Button, IconButton, Seek, Thumb, ...) | `shared/widgets.dart`, `shared/lists.dart`, `shared/sheets.dart`, `shared/icons.dart`, `shared/logo.dart` |
| `pages/*` | `features/home`, `explore`, `library`, `playlist`, `queue`, `search` |
| `NowPlaying`, `LyricsPanel`, `RelatedPanel` | `features/now_playing/now_playing.dart` |
| `SettingsSheet`, `SleepSheet` | `features/settings/settings_sheet.dart` |

Breakpoints are the same: wide from 900 px, the queue lane from 1440 px, page margins 12/24/32/40 px.
The icons are the Svelte icon paths, parsed at run time (`parseSvgPath`).

Not ported (they need Tauri-side services the bridge does not expose yet): the theme editor and theme import/export,
updates, the changelog, the advanced and diagnostics sections, Discord, Last.fm, start with Windows, the mini player
and the flyout.
