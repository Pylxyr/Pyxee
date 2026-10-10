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
handover, prefetch, radio) so the real state machine drives the UI. Demo tracks only. Replaced in milestone 4.
UI-only for now: the volume slider (stored locally; the engine consumes it in milestone 4), the like button
(disabled until the library, milestone 5).

## Layout
Wide (>= 900 px): side rail + content + player bar. Narrow: content + player bar + bottom navigation.
The player bar shows once a track exists; click it (or the chevron) to open Now Playing
(Up next / Lyrics / Related; Lyrics and Related are placeholders).

## Theming and Mica
CSS variables on `<html>`: `data-theme` (dark/light, follows the system unless overridden) and `data-backdrop`.
On Windows 11 the app asks for Mica (`window-vibrancy`); if that succeeds the backend reports `mica` and the page
background becomes transparent with translucent panels. On Windows 10 (or if Mica fails) it stays solid.
The accent is red by default; "Color from album art" derives it from the thumbnail and lightens/darkens it to stay readable.

## No native dialogs
Menus are an in-page `Sheet` (focus trap, Escape, restores focus); messages are `Snackbar`s with Undo.

## Accessibility
Real buttons, `aria-label`s on icon buttons, `aria-pressed` toggles, `role=tab`/`tabpanel`, a real `<input type=range>` for
seeking (arrow keys work, `aria-valuetext` reads "1:23 of 3:42"), visible focus ring, reduced-motion respected for skeletons.

## Keyboard (in-app)
Space play/pause, Left/Right seek 5 s, N next, P previous, Esc close Now Playing. Ignored while typing in a text field;
Space and arrows are left to focused buttons and sliders. Global media keys come with SMTC in milestone 6.

## Not verified here
Everything runs in tests (jsdom) and compiles against Tauri on Linux, but the sandbox has no Windows WebView2, so the
real window, Mica and DPI behaviour need a first run on your PC.
