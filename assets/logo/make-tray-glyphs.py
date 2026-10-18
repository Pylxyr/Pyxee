#!/usr/bin/env python3
"""Transport glyphs for the notification-area buttons (32x32): the white thumbnail-button glyphs with a thin dark outline,
so they stay readable on both the dark and the light Windows taskbar -> crates/app/icons/tray-buttons/.
Needs Pillow; run after make-thumb-glyphs.py.   python3 assets/logo/make-tray-glyphs.py"""
from pathlib import Path
from PIL import Image, ImageChops, ImageFilter

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "crates/app/icons/thumb"
OUT = ROOT / "crates/app/icons/tray-buttons"

OUT.mkdir(parents=True, exist_ok=True)
for name in ("prev", "play", "pause", "next"):
    glyph = Image.open(SRC / f"{name}.png").convert("RGBA")
    alpha = glyph.getchannel("A")
    # Grow the shape by two pixels for the outline, softened slightly so it does not look jagged at 16 px.
    halo = alpha.filter(ImageFilter.MaxFilter(5)).filter(ImageFilter.GaussianBlur(0.6))
    outline = Image.new("RGBA", glyph.size, (20, 20, 24, 0))
    outline.putalpha(halo.point(lambda v: min(255, int(v * 1.3))))
    out = Image.alpha_composite(outline, glyph)
    out.save(OUT / f"{name}.png")
    print("wrote", OUT / f"{name}.png")
