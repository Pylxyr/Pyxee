# Pyxee on Linux

**Linux packaging is pending.** The earlier Linux guide described the Tauri build and an Arch `PKGBUILD`; both are gone
with the Tauri removal (they are in the `tauri-final` tag), and no replacement has been built or tested yet. There is no
installable Linux package, and the Flutter app has not been built or run on Linux. Nothing here should be read as
supported until a Flutter Linux build has actually been run.

## What works today

The Rust side builds, passes clippy and passes its tests on Linux (the `linux` job in CI). On Debian or Ubuntu:

```
sudo apt install build-essential pkg-config nasm make diffutils libasound2-dev
cargo xtask ffmpeg                      # the minimal static FFmpeg, once
cargo test --workspace
cargo build -p pyxee-bridge --release   # target/release/libpyxee_bridge.so
```

On Arch: `sudo pacman -S --needed base-devel git nasm alsa-lib`. Sound goes through ALSA, which PipeWire provides through
`pipewire-alsa`. The bridge announces itself to the desktop over MPRIS (D-Bus), so media keys and the media widget of KDE and
GNOME can control it once a UI is attached.

## What a Linux build still needs

- A Flutter Linux build of `ui_flutter` that loads `libpyxee_bridge.so` (the native assets hook currently targets
  `pyxee_bridge.dll`), tried on a real machine.
- A package or installer (an Arch `PKGBUILD`, an AppImage, or a Flatpak), the desktop entry and icons (`assets/icons/`), and a
  way to bundle yt-dlp.
- Checks of the tray, the window frame and the fonts (`Bahnschrift` and `Segoe UI Variable Text` fall back to the system font).

## Where Pyxee keeps its files

Everything is in `~/.local/share/dev.pyxee.app`:

| Item | What it is |
|---|---|
| `session.json` | Your queue and what was playing |
| `.env` | Your settings from [advanced.md](advanced.md) |
| `tools/` | A yt-dlp that Settings > Updates installed, which wins over the one that came with Pyxee |

The log goes to the terminal (set `PYXEE_LOG=debug`); there is no log file in this version.
