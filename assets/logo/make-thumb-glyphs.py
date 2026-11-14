#!/usr/bin/env python3
"""White transport glyphs (32x32, transparent) for the taskbar thumbnail buttons -> crates/app/icons/thumb/.
Needs Pillow.   python3 assets/logo/make-thumb-glyphs.py"""
from pathlib import Path
from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parents[2] / "crates/app/icons/thumb"
S, K = 32, 8  # size, supersampling


def canvas():
    im = Image.new("RGBA", (S * K, S * K), (0, 0, 0, 0))
    return im, ImageDraw.Draw(im)


def p(*pts):
    return [(x * K, y * K) for x, y in pts]


def save(im, name):
    OUT.mkdir(parents=True, exist_ok=True)
    im.resize((S, S), Image.LANCZOS).save(OUT / f"{name}.png")


W = (255, 255, 255, 255)
im, d = canvas(); d.polygon(p((10, 6), (10, 26), (26, 16)), fill=W); save(im, "play")
im, d = canvas(); d.rectangle(p((8, 6), (13.5, 26)), fill=W); d.rectangle(p((18.5, 6), (24, 26)), fill=W); save(im, "pause")
im, d = canvas(); d.rectangle(p((7, 7), (10.5, 25)), fill=W); d.polygon(p((25, 7), (25, 25), (11.5, 16)), fill=W); save(im, "prev")
im, d = canvas(); d.rectangle(p((21.5, 7), (25, 25)), fill=W); d.polygon(p((7, 7), (7, 25), (20.5, 16)), fill=W); save(im, "next")
print("wrote", sorted(f.name for f in OUT.glob("*.png")))
