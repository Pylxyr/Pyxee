# Pyxee on Linux

Pyxee runs on Linux as its own build: the same player, the same library and the same settings as on Windows, compiled
for your machine and set up the way Linux desktops work. It is made for Arch Linux and CachyOS first. Other
distributions can build it from source (see [development.md](development.md)).

## Install (Arch Linux, CachyOS)
```
sudo pacman -S --needed base-devel git
git clone https://github.com/Pylxyr/Pyxee.git
cd Pyxee/packaging/arch
updpkgsums
makepkg -si
```
`makepkg` builds Pyxee from the newest release and installs it with pacman, so `pacman -R pyxee` removes it again. The
first build takes a while (roughly half an hour on a fast PC): it builds Pyxee's small audio decoder and then the program
itself, both for your CPU. Pyxee needs `webkit2gtk-4.1`, `gtk3`, `libayatana-appindicator`, `alsa-lib`, `pipewire-alsa`
and `yt-dlp`; `makepkg` installs them. `deno` is optional (see "JavaScript runtime" in the README).

The package is built for the CPU it is built on (`native`). To build one that also runs on other PCs, use
`_march=x86-64-v3 makepkg -si` (CachyOS's own baseline) or `_march=x86-64`.

## Updates
Pyxee does not replace itself on Linux. To update, pull the repository and run `makepkg -si` again. **Settings** ->
**Updates** still tells you when a newer version is out, and it keeps yt-dlp (the part that finds your music) up to
date: Pyxee downloads yt-dlp's own Linux program into its data folder and uses that, falling back to the `yt-dlp` from
your package manager.

## What is different from Windows
* **Window frame.** The main window uses your desktop's own title bar. **Settings** -> **Borderless window** switches
  to Pyxee's slim bar, but a window without a frame may have no edges to resize by.
* **Tray.** The icon's menu has Open Pyxee, Play / Pause, Previous, Next, Mini player and Quit. On most Linux desktops
  a click on the icon opens the menu, so the Windows option "clicking the icon opens the mini controls" does not exist
  here, and neither do the previous/play/next buttons beside the icon. KDE Plasma shows the icon out of the box; GNOME
  needs the "AppIndicator and KStatusNotifierItem Support" extension.
* **Media keys and the media widget.** Pyxee announces itself to the desktop (MPRIS), so the media keys and the media
  widget of KDE and GNOME can control it.
* **Sound.** Pyxee plays through ALSA, which PipeWire (the default on CachyOS) provides through `pipewire-alsa`.
* **Last.fm.** The Last.fm sign-in is kept in Pyxee's settings file instead of a system credential store.
* **Updates, installer, SmartScreen.** None of these apply.

## Where Pyxee keeps its files
In `~/.local/share/dev.pyxee.app`: the library and queue (`session.json`), the settings, the `.env` file of
[Advanced settings](advanced.md), a `tools` folder with the updated yt-dlp, and `logs/pyxee.log`. **Settings** ->
**Diagnostics** -> **Open log folder** opens it.

## If something goes wrong
* **Nothing plays, or the sound is silent.** Check that `pipewire-alsa` is installed and that another program is not
  holding the sound device exclusively. Send me the last lines of `logs/pyxee.log`.
* **The window stays blank.** WebKitGTK sometimes draws nothing with some graphics drivers. Start Pyxee once with
  `WEBKIT_DISABLE_DMABUF_RENDERER=1 pyxee-app`; if that fixes it, tell me which graphics card you have.
* **No tray icon on GNOME.** Install the AppIndicator extension named above.
