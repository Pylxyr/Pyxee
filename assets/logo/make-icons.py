#!/usr/bin/env python3
"""Regenerates every icon from assets/logo/logo.svg (full mark) and tray.svg (simplified for 16-24 px).

Needs: rsvg-convert (librsvg) and Pillow.   python3 assets/logo/make-icons.py
Outputs (committed): crates/app/icons/{32x32,128x128,128x128@2x,icon}.png, icon.ico,
tray-{16,24,32}.png, tray.ico, and assets/logo/png/logo-1024.png.
"""
import io
import struct
import subprocess
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
LOGO = ROOT / "assets/logo/logo.svg"
TRAY = ROOT / "assets/logo/tray.svg"
ICONS = ROOT / "crates/app/icons"


def render(svg: Path, size: int) -> bytes:
    return subprocess.run(
        ["rsvg-convert", "-w", str(size), "-h", str(size), str(svg)], check=True, capture_output=True
    ).stdout


def png(svg: Path, size: int, out: Path) -> None:
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(render(svg, size))


def ico(frames: list[tuple[int, bytes]], out: Path) -> None:
    """Writes an .ico whose entries are PNG images (Windows Vista and later)."""
    head = struct.pack("<HHH", 0, 1, len(frames))
    offset = 6 + 16 * len(frames)
    entries, data = b"", b""
    for size, blob in frames:
        w = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", w, w, 0, 0, 1, 32, len(blob), offset + len(data))
        data += blob
    out.write_bytes(head + entries + data)


def main() -> None:
    # Small frames use the simplified mark; from 32 px up the full one.
    app = [(s, render(TRAY if s <= 24 else LOGO, s)) for s in (16, 24, 32, 48, 64, 128, 256)]
    ico(app, ICONS / "icon.ico")
    ico([(s, render(TRAY, s)) for s in (16, 24, 32)], ICONS / "tray.ico")
    for s in (16, 24, 32):
        png(TRAY, s, ICONS / f"tray-{s}.png")
    png(LOGO, 32, ICONS / "32x32.png")
    png(LOGO, 128, ICONS / "128x128.png")
    png(LOGO, 256, ICONS / "128x128@2x.png")
    png(LOGO, 512, ICONS / "icon.png")
    png(LOGO, 1024, ROOT / "assets/logo/png/logo-1024.png")
    # Self-check: every file decodes and is square and transparent at the corners.
    for p in sorted(ICONS.glob("*.png")):
        im = Image.open(p).convert("RGBA")
        assert im.width == im.height and im.getpixel((0, 0))[3] == 0, p
    for p in (ICONS / "icon.ico", ICONS / "tray.ico"):
        im = Image.open(p)
        print(p.name, sorted(im.info.get("sizes", [])))


if __name__ == "__main__":
    main()
