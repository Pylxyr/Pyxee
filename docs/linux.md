# Pyxee on Linux

Pyxee on Linux is the same player, library and settings as on Windows. It is built on your own machine from source, so
it is tuned for your CPU and installed the way your distribution installs software. This guide takes you from nothing to
a running Pyxee, with every step explained, and has extra options for advanced users along the way.

**On this page**

- [Which guide do I follow?](#which-guide-do-i-follow)
- [Arch Linux and CachyOS](#arch-linux-and-cachyos)
- [Other distributions (build from source)](#other-distributions-build-from-source)
- [Updating](#updating)
- [Uninstalling](#uninstalling)
- [What is different from Windows](#what-is-different-from-windows)
- [Where Pyxee keeps its files](#where-pyxee-keeps-its-files)
- [Troubleshooting](#troubleshooting)

## Which guide do I follow?

| Your system | Follow |
|---|---|
| Arch Linux, CachyOS, or another distribution that uses `pacman` | [Arch Linux and CachyOS](#arch-linux-and-cachyos) |
| Debian, Ubuntu, Fedora or anything else | [Other distributions](#other-distributions-build-from-source) |

If you are not sure, run `cat /etc/os-release` in a terminal and read the `NAME` line.

What you need in every case:

- A 64-bit PC (x86_64). The Arch package is built for x86_64 only; other CPU types are untested.
- An internet connection. The build downloads the program's source, its libraries and the audio decoder.
- Time and disk space. The first build takes roughly half an hour on a fast PC, and longer on a slower one. Plan for
  several gigabytes of free disk space while it builds.
- A terminal. On most desktops you can open one with `Ctrl+Alt+T`, or search the application menu for "Terminal" or
  "Konsole".

Two things worth knowing before you start:

- The build is long but quiet. Long stretches with no new output are normal.
- When a terminal asks for your password, nothing appears while you type. That is expected; type it and press Enter.

## Arch Linux and CachyOS

Other Arch-based distributions that use `pacman` should work with the same steps.

### Step by step

**1. Install the build tools.**

```
sudo pacman -S --needed base-devel git
```

`base-devel` provides `makepkg` and the compilers, and `git` downloads the package recipe.

**2. Download the package recipe.**

```
git clone https://github.com/Pylxyr/Pyxee.git ~/Pyxee
cd ~/Pyxee/packaging/arch
```

This puts a `Pyxee` folder in your home folder (`~`) and moves into the part of it that holds a file called `PKGBUILD`,
the recipe that tells `makepkg` how to build Pyxee. Keep the folder: updating later reuses it. `makepkg` downloads the
program's source itself, from a tagged release.

**3. Build and install.**

```
makepkg -sic
```

The three letters mean: `s` installs any build tools that are missing, `i` installs Pyxee when it is built, and `c`
deletes the work folders afterwards. Do not put `sudo` in front of this command; `makepkg` refuses to run as root and
asks for your password only when it needs to install something. Answer `Y` when it offers to install missing
dependencies. The build goes through four stages:

1. It downloads Pyxee's source.
2. It builds the user interface (the page you see in the window).
3. It builds Pyxee's small audio decoder, a minimal FFmpeg (about 10 minutes on one core).
4. It compiles Pyxee itself.

When it finishes, `pacman` installs the package. The dependencies Pyxee needs (`webkit2gtk-4.1`, `gtk3`,
`libayatana-appindicator`, `alsa-lib`, `pipewire-alsa` and `yt-dlp`) are installed automatically.

You do not need to clean up: the build output (several gigabytes) is deleted by the recipe as soon as the package is
made, and `-c` removes the work folders. What stays is described in [Freeing disk space](#freeing-disk-space).

**4. Start Pyxee.**

Search for "Pyxee" in your application menu, or run `pyxee-app` in a terminal.

**5. Optional: install a JavaScript runtime.** yt-dlp uses one to keep every audio format available, and Pyxee shows a
warning without one. See "A warning about a JavaScript runtime" in the [README](../README.md#if-something-goes-wrong).

```
sudo pacman -S deno
```

Because Pyxee was installed with `pacman`, you can check it with `pacman -Qi pyxee` and remove it with
`sudo pacman -R pyxee`.

### Options for advanced users

**Choose the CPU the build targets.** By default the package is tuned for the CPU it is built on (`native`), which is
the fastest option but means the package only runs on that CPU or a newer one. To build a package that also runs on
other PCs, set `_march`:

```
_march=x86-64-v3 makepkg -sic     # CachyOS's own baseline; needs a CPU with AVX2
_march=x86-64 makepkg -sic        # runs on any 64-bit PC
```

**Build now, install later.** Build the package without installing it, then install the resulting file with `pacman`
(on this PC or another one; use a non-`native` `_march` for another PC):

```
makepkg -sc
sudo pacman -U pyxee-*.pkg.tar.zst
```

**Which version do I get?** `makepkg` builds the release named on the `pkgver=` line at the top of `PKGBUILD`. Compare
it with the newest version on the [Releases page](https://github.com/Pylxyr/Pyxee/releases). To build a different
release, edit `pkgver` and run `makepkg -sic` again. Releases older than 0.5.1 cannot be built with this recipe,
because they do not have the Linux build profile it uses.

**Checksums (optional).** The recipe ships with `SKIP` in place of checksums, so the download is not verified.
`updpkgsums` (install it with `sudo pacman -S pacman-contrib`) downloads the release once and writes its checksum into
`PKGBUILD`, which then catches a damaged re-download. The checksum comes from the same download, so it does not prove
the file is genuine. It changes `PKGBUILD`, so run `git restore packaging/arch/PKGBUILD` before the next `git pull`.

**What gets installed.**

| File | Location |
|---|---|
| The program | `/usr/bin/pyxee-app` |
| Menu entry | `/usr/share/applications/pyxee.desktop` |
| Icons | `/usr/share/icons/hicolor/` (32x32 and 128x128) |
| Licences | `/usr/share/licenses/pyxee/` |

### Freeing disk space

You do not have to do anything for the big files: the recipe deletes the build output (the same as `cargo clean`, plus
the npm packages) as soon as the package is made, and `makepkg -c` removes the work folders. Two small things stay in
`~/Pyxee/packaging/arch`: the source download (`pyxee-*.tar.gz`) and the finished package (`pyxee-*.pkg.tar.zst`, which
you can keep to reinstall without rebuilding). To remove both:

```
cd ~/Pyxee/packaging/arch
rm -f pyxee-*.tar.gz pyxee-*.pkg.tar.zst
```

The Rust and npm download caches (`~/.cargo` and `~/.npm`) are kept on purpose, so the next update downloads less. If you
need that space too: `rm -rf ~/.cargo/registry ~/.cargo/git ~/.npm/_cacache`.

## Other distributions (build from source)

There is no installable package for other distributions yet, and Tauri's bundler does not produce a Linux package for
Pyxee. You build the program and run it, and can optionally copy it into place for your user.

### Step by step

**1. Install the system packages.**

Debian and Ubuntu (this is the list the project's CI installs on Ubuntu):

```
sudo apt update
sudo apt install -y build-essential git curl pkg-config nasm make \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev libdbus-1-dev libssl-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev libxdo-dev
```

Fedora (package names follow Tauri's own prerequisites and have not been tested by this project; if a name is not
found, use `dnf search` to find its equivalent):

```
sudo dnf install gcc gcc-c++ make git curl nasm pkgconf-pkg-config \
  webkit2gtk4.1-devel gtk3-devel libayatana-appindicator-gtk3-devel librsvg2-devel \
  alsa-lib-devel dbus-devel openssl-devel libsoup3-devel \
  javascriptcoregtk4.1-devel libxdo-devel
```

Another distribution: install the equivalents of those packages. The libraries that matter are WebKitGTK 4.1, GTK 3,
AppIndicator, ALSA, D-Bus, OpenSSL and librsvg (with their development headers), plus a C compiler, `make`, `nasm`,
`git` and `pkg-config`. Your distribution must ship WebKitGTK 4.1.

You also want `yt-dlp` from your package manager. Pyxee downloads and updates its own copy, and uses the system one as a
fallback, so having it installed means music works from the first start.

**2. Install Rust.** Use [rustup](https://rustup.rs/):

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Close and reopen the terminal afterwards. You do not need to pick a version: the repository contains a
`rust-toolchain.toml`, so rustup downloads Rust 1.91 by itself the first time you build inside the folder.

**3. Install Node.js 22 or newer.** Check with `node --version`. If your distribution's package is older, install a
current version from [nodejs.org](https://nodejs.org/) or with a version manager such as
[nvm](https://github.com/nvm-sh/nvm).

**4. Download the source.**

```
git clone https://github.com/Pylxyr/Pyxee.git
cd Pyxee
```

The default branch holds the newest work. For a tested release, switch to its tag (replace `v0.5.1` with the newest
version on the [Releases page](https://github.com/Pylxyr/Pyxee/releases)):

```
git checkout v0.5.1
```

**5. Build the user interface.**

```
npm ci --prefix ui
npm run build --prefix ui
```

**6. Build the audio decoder.** This downloads FFmpeg's source and builds a minimal, static copy (about 10 minutes on
one core). It only needs to be done once.

```
cargo xtask ffmpeg
```

**7. Build Pyxee.**

```
cargo build --profile release-linux --locked -p pyxee-app --features tauri/custom-protocol
```

This is the slowest step. `--features tauri/custom-protocol` makes the program show the page you built in step 5;
without it the window tries to load a development server and stays blank.

**8. Run it.**

```
./target/release-linux/pyxee-app
```

**9. Optional: install it for your user.** This copies the program, the menu entry and the icons into your home folder,
so Pyxee appears in your application menu:

```
install -Dm755 target/release-linux/pyxee-app ~/.local/bin/pyxee-app
install -Dm644 packaging/arch/pyxee.desktop ~/.local/share/applications/pyxee.desktop
install -Dm644 crates/app/icons/128x128.png ~/.local/share/icons/hicolor/128x128/apps/pyxee.png
install -Dm644 crates/app/icons/32x32.png ~/.local/share/icons/hicolor/32x32/apps/pyxee.png
sed -i "s|^Exec=.*|Exec=$HOME/.local/bin/pyxee-app|" ~/.local/share/applications/pyxee.desktop
```

The last line points the menu entry at the full path, so it works even if `~/.local/bin` is not on your `PATH`. If Pyxee
does not show up in the menu straight away, log out and back in. To install for all users instead, copy the program to
`/usr/local/bin` with `sudo install -Dm755`, and the other files under `/usr/local/share`.

**10. Optional: install a JavaScript runtime.** Install `nodejs` with your package manager, or Deno from
[deno.com](https://deno.com/). See the [README](../README.md#if-something-goes-wrong) for why.

### Options for advanced users

**Target a specific CPU.** For a program tuned to the build machine, build FFmpeg and Pyxee with the same target. Use
`x86-64-v3` instead of `native` for a build that runs on other modern PCs. FFmpeg rebuilds automatically when its flags
change.

```
PYXEE_FF_MARCH=native cargo xtask ffmpeg
RUSTFLAGS="-C target-cpu=native" \
  cargo build --profile release-linux --locked -p pyxee-app --features tauri/custom-protocol
```

**Build faster.** The `release-linux` profile uses whole-program optimisation, which gives a smaller, faster program
but a slow build. For a quicker build, use `--release` instead of `--profile release-linux`. The program is then in
`target/release/pyxee-app`.

**Use an FFmpeg build from another place.** Set `PYXEE_FFMPEG_DIR` to the folder that holds the libraries; otherwise
`target/ffmpeg/<host triple>` is used.

**Develop and test.** Running the test suite, the development server and the code layout are in
[development.md](development.md). Settings for yt-dlp, cookies, logging and more are in
[advanced.md](advanced.md).

## Updating

Pyxee does not replace itself on Linux. **Settings** -> **Updates** still tells you when a newer version is out, and it
keeps yt-dlp (the part that finds your music) up to date: Pyxee downloads yt-dlp's own Linux program into its data
folder and uses that, falling back to the `yt-dlp` from your package manager.

**Arch Linux and CachyOS.** Updating is two short steps. Open a terminal and copy the lines in.

**1. Download the newest recipe and see if there is something new.**

```
cd ~/Pyxee
git restore packaging/arch/PKGBUILD
git pull
pacman -Q pyxee
grep ^pkgver packaging/arch/PKGBUILD
```

The last two lines print the version you have installed (for example `pyxee 0.5.1-1`) and the version the recipe will
build (for example `pkgver=0.5.1`). If the numbers are the same, you are up to date and can stop here. If the second is
newer, go on to step 2. (`git restore` undoes any edit you made to `PKGBUILD`, so that `git pull` cannot be blocked.)

**2. Build and install the new version.**

```
cd ~/Pyxee/packaging/arch
makepkg -sic
```

This takes as long as the first build, and it cleans up after itself the same way. Your library, queue and settings are
kept.

Prefer one line? This does both steps and stops at the first problem:

```
cd ~/Pyxee && git restore packaging/arch/PKGBUILD && git pull && cd packaging/arch && makepkg -sic
```

**Other distributions.** Pull, then repeat steps 5 to 7 (and 9 if you installed it for your user):

```
cd Pyxee
git pull
```

Step 6 is skipped automatically when the audio decoder has not changed ("FFmpeg already built").

## Uninstalling

**Arch Linux and CachyOS:**

```
sudo pacman -R pyxee
```

**Installed for your user (other distributions):**

```
rm ~/.local/bin/pyxee-app ~/.local/share/applications/pyxee.desktop
rm ~/.local/share/icons/hicolor/128x128/apps/pyxee.png ~/.local/share/icons/hicolor/32x32/apps/pyxee.png
```

Neither removes your library, queue and settings, which live in the folder described in
[Where Pyxee keeps its files](#where-pyxee-keeps-its-files). Delete `~/.local/share/dev.pyxee.app` only if you want
those gone too; this cannot be undone.

## What is different from Windows

- **Window frame.** The main window uses your desktop's own title bar. **Settings** -> **Borderless window** switches to
  Pyxee's slim bar, but a window without a frame may have no edges to resize by.
- **Tray.** The icon's menu has Open Pyxee, Play / Pause, Previous, Next, Mini player and Quit. On most Linux desktops a
  click on the icon opens the menu, so the Windows option "clicking the icon opens the mini controls" does not exist
  here, and neither do the previous/play/next buttons beside the icon. KDE Plasma shows the icon out of the box; GNOME
  needs the "AppIndicator and KStatusNotifierItem Support" extension.
- **Media keys and the media widget.** Pyxee announces itself to the desktop (MPRIS), so the media keys and the media
  widget of KDE and GNOME can control it.
- **Sound.** Pyxee plays through ALSA, which PipeWire (the default on CachyOS) provides through `pipewire-alsa`.
- **Last.fm.** The Last.fm sign-in is kept in Pyxee's settings file instead of a system credential store.
- **Updates, installer, SmartScreen.** None of these apply.

## Where Pyxee keeps its files

Everything is in `~/.local/share/dev.pyxee.app`:

| Item | What it is |
|---|---|
| `session.json` | Your library and queue |
| `.env` | Your settings from [Advanced settings](advanced.md) |
| `tools/` | The updated yt-dlp |
| `logs/pyxee.log` | The log |

**Settings** -> **Diagnostics** -> **Open log folder** opens the log folder for you.

## Troubleshooting

**`updpkgsums: command not found`.** This optional step needs `pacman-contrib` (`sudo pacman -S pacman-contrib`). You can
also skip it; the build works without it.

**`makepkg` says it must not run as root.** Run `makepkg -sic` as your normal user, without `sudo`. It asks for your
password itself when it installs.

**`error: profile 'release-linux' is not defined`.** The recipe is set to build a release that is too old to have the
Linux build profile (0.5.0 or earlier). Update the repository and start again from a clean folder:

```
cd ~/Pyxee
git restore packaging/arch/PKGBUILD
git pull
cd packaging/arch
rm -rf src pkg
makepkg -sic
```

**The build stops with `Killed`, or the computer freezes near the end.** The last compile step uses a lot of memory.
Close other programs and try again. If it keeps happening, add swap space or enable zram.

**The build stops with a download error.** The build downloads Pyxee's source, npm packages, Rust crates and FFmpeg's
source. Check your connection and run the same command again; finished work is kept.

**`error: 'nasm' is required to build FFmpeg but was not found on PATH`** (other distributions). Install `nasm`, `make`
and `git` (see step 1) and run `cargo xtask ffmpeg` again.

**`FFmpeg source is at ..., expected ...`.** A leftover FFmpeg source folder is at the wrong version. Delete the folder
the message names and run `cargo xtask ffmpeg` again.

**Nothing plays, or the sound is silent.** Check that `pipewire-alsa` (or your distribution's ALSA plugin for PipeWire)
is installed and that another program is not holding the sound device exclusively.

**The window stays blank.** WebKitGTK sometimes draws nothing with some graphics drivers. Start Pyxee once with
`WEBKIT_DISABLE_DMABUF_RENDERER=1 pyxee-app`. If that fixes it, mention which graphics card you have when you report it.
If you built from source without `--features tauri/custom-protocol`, rebuild with it (step 7).

**No tray icon on GNOME.** Install the "AppIndicator and KStatusNotifierItem Support" extension.

**Songs will not load.** See "If something goes wrong" in the [README](../README.md#if-something-goes-wrong).

**Still stuck?** [Open an issue](https://github.com/Pylxyr/Pyxee/issues) with your distribution and desktop, your
graphics card if the window is the problem, and the last lines of `logs/pyxee.log`.
