# The Flutter app (`ui_flutter`)

Pyxee's window is a Flutter desktop app. It talks to the Rust core through `crates/bridge`, a C interface that takes
requests as JSON text and pushes events back to a callback. Nothing else of the old shell is used.

## Folders (`ui_flutter/lib`)

| Folder | What is in it |
|---|---|
| `main.dart` | Starts the bridge, the window and the tray, reads the saved settings, then runs the app |
| `core/bridge` | `bridge.dart` is the typed interface, `bridge_native.dart` loads `pyxee_bridge.dll` (or `libpyxee_bridge.so`) with `dart:ffi` |
| `core/models` | The data the core sends (songs, the player state, the library), as plain Dart classes read from JSON |
| `core/state` | Riverpod providers: the player, the library, the position, the sleep timer, the data for each page (home, explore, lyrics, search ...), and the `CoreApi` the UI calls |
| `core/theme` | `prefs.dart` (settings and custom themes, saved through the core), `art_color.dart` (reads a colour from the cover art) |
| `app/theme` | `colors.dart` (the 22 built-in themes, `paletteFromArt`, the contrast rules), `px_theme.dart` (`PxColors`, the type ramp, the Material theme) |
| `app/shell` | The window frame: `shell.dart` (layout, shortcuts, toasts), `top_bar.dart`, `title_bar.dart`, `navigation.dart`, `deck.dart` (player deck, dock and queue lane) |
| `app/state` | `ui_state.dart`: the page you are on, Now Playing open or closed, toasts, search suggestions |
| `shared` | The building blocks: `widgets.dart` (`PxButton`, `PxIconButton`, `PxSegmented`, `PxSeek`, `PxPageHeader`, `PxScrollPage` ...), `lists.dart` (track rows and lists, drag to reorder), `sheets.dart`, `icons.dart`, `logo.dart` |
| `features` | One folder per page: `home`, `explore`, `library`, `playlist`, `queue`, `search`, plus `now_playing` and `settings` (including the theme editor) |
| `desktop` | `desktop_host.dart` (tray, close to tray, quit, window material) and `hwnd.dart` |

Reuse the `Px*` widgets and read colours with `context.px` instead of building new styles; import Flutter's Material
widgets from `package:material_ui/material_ui.dart`.

## Layout

The window changes layout at fixed widths: wide (player deck at the bottom) from 900 px, the queue lane beside the page
from 1440 px, and a dock plus bottom bar below 900 px. Page margins are 12, 24, 32 and 40 px. The icons are SVG paths
parsed at run time (`parseSvgPath`), so they take the text colour.

## Running it

From the repo root, once (see [development.md](development.md)): `cargo xtask ffmpeg`, `cargo xtask tools`,
`cargo build -p pyxee-bridge --release`. Then in `ui_flutter`: `flutter pub get`, `flutter run -d windows`. The native
assets hook (`hook/build.dart`) only copies the built bridge library next to the app, so rebuild the bridge after every
Rust change.

## Not in this version

Settings > Diagnostics and a log file, Last.fm, start with Windows, notifications, the mini player, the tray flyout, taskbar
buttons and the jump list, and single instance. See [migration.md](migration.md) for
where each lived and what bringing it back needs.
