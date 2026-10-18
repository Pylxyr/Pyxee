#!/usr/bin/env bash
# Builds the minimal static FFmpeg (libavformat/avcodec/swresample/avutil) that Pyxee links in-process.
# Usage: scripts/build-ffmpeg.sh <install-prefix> [work-dir]
#
# One script for Linux (tested in CI-less sandboxes) and Windows (MSYS2 bash with the MSVC tools on
# PATH, PYXEE_FF_TOOLCHAIN=msvc; `cargo xtask ffmpeg` sets this up). LGPL only: no --enable-gpl, no
# --enable-nonfree, no external libraries. Audio decode only: no programs, filters, devices, network.
set -euo pipefail

# Pinned by commit, not tag: a tag can move, a commit cannot.
FFMPEG_TAG="n8.1.3"
FFMPEG_COMMIT="1041abdc962f4cc4f394aa8de9dc5236c0c3b9e7"
FFMPEG_URL="https://github.com/FFmpeg/FFmpeg.git"

prefix="${1:?usage: build-ffmpeg.sh <install-prefix> [work-dir]}"
work="${2:-}"
# MSYS2: turn Windows paths (D:/repo/...) into MSYS ones (/d/repo/...), which configure and make handle best.
if command -v cygpath >/dev/null 2>&1; then
  prefix="$(cygpath -u "$prefix")"
  if [ -n "$work" ]; then work="$(cygpath -u "$work")"; fi
fi
work="${work:-${prefix}.build}"
src="$work/src"
mkdir -p "$prefix" "$work"

flags=(
  --disable-everything --disable-autodetect --disable-programs --disable-doc
  --disable-avdevice --disable-avfilter --disable-swscale --disable-network
  --disable-debug --disable-shared --enable-static --enable-pic
  --enable-avcodec --enable-avformat --enable-avutil --enable-swresample
  # HTTP stays in Rust (HttpRangeSource feeds libavformat through callbacks), so no protocols at all.
  --enable-demuxer=matroska,mov,ogg,mp3,flac,wav,aac,aiff
  --enable-parser=opus,aac,mpegaudio,flac,vorbis
  --enable-decoder=opus,aac,mp3float,flac,vorbis,alac,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s24be,pcm_s32le,pcm_s32be,pcm_f32le,pcm_f64le,pcm_u8
)
if [ "${PYXEE_FF_TOOLCHAIN:-}" = "msvc" ]; then
  flags+=(--toolchain=msvc)
  # MSYS2's coreutils `link.exe` would shadow the MSVC linker (FFmpeg's documented MSVC setup moves it aside).
  if [ -f /usr/bin/link.exe ]; then mv /usr/bin/link.exe /usr/bin/link.exe.bak; fi
fi

stamp="$(printf '%s\n' "$FFMPEG_COMMIT" "${flags[@]}" | sha256sum | cut -d' ' -f1)"
if [ -f "$prefix/pyxee-ffmpeg.stamp" ] && [ "$(cat "$prefix/pyxee-ffmpeg.stamp")" = "$stamp" ]; then
  echo "FFmpeg already built at $prefix (flags and commit unchanged)"
  exit 0
fi

if [ ! -d "$src/.git" ]; then
  git clone --depth 1 --branch "$FFMPEG_TAG" "$FFMPEG_URL" "$src"
fi
got="$(git -C "$src" rev-parse HEAD)"
if [ "$got" != "$FFMPEG_COMMIT" ]; then
  echo "error: FFmpeg source is at $got, expected $FFMPEG_COMMIT. Delete $src and retry." >&2
  exit 1
fi

mkdir -p "$work/obj"
cd "$work/obj"
"$src/configure" --prefix="$prefix" "${flags[@]}"
# Refuse anything that is not LGPL-only.
if grep -q '^#define CONFIG_GPL 1' config.h || grep -q '^#define CONFIG_NONFREE 1' config.h; then
  echo "error: the build is not LGPL-only" >&2
  exit 1
fi
make -j"$(nproc 2>/dev/null || echo 2)"
make install
echo "$stamp" > "$prefix/pyxee-ffmpeg.stamp"
echo "FFmpeg $FFMPEG_TAG installed to $prefix"
