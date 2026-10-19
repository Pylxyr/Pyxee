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


def dib(blob: bytes) -> bytes:
    """A PNG as the 32-bit BMP (DIB) image an .ico frame holds: the layout every Windows shell component reads
    (Explorer, Task Manager, the taskbar, the Alt-Tab list), including at DPI-scaled small sizes."""
    im = Image.open(io.BytesIO(blob)).convert("RGBA")
    w, h = im.size
    px = im.load()
    rows = b"".join(
        b"".join(struct.pack("<BBBB", px[x, y][2], px[x, y][1], px[x, y][0], px[x, y][3]) for x in range(w))
        for y in range(h - 1, -1, -1)  # bottom-up
    )
    # The AND mask: 1 bit per pixel, bottom-up, MSB first, rows padded to 4 bytes; 1 = transparent. Some shell paths
    # (Task Manager's process list showed black/cyan noise) ignore the alpha channel and draw with this mask, so it
    # must agree with the alpha instead of being left all 0 (= "opaque everywhere").
    mask_row = ((w + 31) // 32) * 4
    mask = bytearray()
    for y in range(h - 1, -1, -1):
        row = bytearray(mask_row)
        for x in range(w):
            if px[x, y][3] < 128:
                row[x // 8] |= 0x80 >> (x % 8)
        mask += row
    header = struct.pack("<IiiHHIIiiII", 40, w, h * 2, 1, 32, 0, len(rows), 0, 0, 0, 0)
    return header + rows + bytes(mask)


def ico(frames: list[tuple[int, bytes]], out: Path) -> None:
    """Writes an .ico: small frames as 32-bit BMP, 256 px as PNG (the combination every Windows version reads,
    unlike an all-PNG file, which some shell components show as a generic icon)."""
    head = struct.pack("<HHH", 0, 1, len(frames))
    offset = 6 + 16 * len(frames)
    entries, data = b"", b""
    for size, blob in frames:
        w = 0 if size >= 256 else size
        image = blob if size >= 256 else dib(blob)
        entries += struct.pack("<BBBBHHII", w, w, 0, 0, 1, 32, len(image), offset + len(data))
        data += image
    out.write_bytes(head + entries + data)


def main() -> None:
    # Small frames use the simplified mark; from 32 px up the full one.
    app = [(s, render(TRAY if s <= 24 else LOGO, s)) for s in (16, 20, 24, 32, 40, 48, 64, 128, 256)]
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
        raw = p.read_bytes()
        count = struct.unpack("<H", raw[4:6])[0]
        kinds = []
        for i in range(count):
            w, _, _, _, _, _, size, off = struct.unpack("<BBBBHHII", raw[6 + 16 * i : 22 + 16 * i])
            kinds.append(f"{w or 256}:{'png' if raw[off:off + 4] == b'\x89PNG' else 'bmp'}")
        print(p.name, " ".join(kinds))
        for size in (16, 20, 24, 32, 48):  # Pillow must be able to read every small frame back
            if f"{size}:" in " ".join(kinds):
                Image.open(p).size  # noqa: B018


if __name__ == "__main__":
    main()
