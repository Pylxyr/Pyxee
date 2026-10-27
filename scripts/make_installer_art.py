#!/usr/bin/env python3
"""Draws the two pictures the NSIS installer shows, from the app icon.

    python scripts/make_installer_art.py

Writes crates/app/icons/installer/header.bmp (150x57, top right of the pages) and sidebar.bmp (164x314, the welcome
and finish pages). NSIS wants 24-bit BMPs without transparency. Needs Pillow. The results are committed, so this is
only run when the icon changes.
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
ICON = ROOT / "crates/app/icons/icon.png"
OUT = ROOT / "crates/app/icons/installer"
SCALE = 4  # drawn four times as large, then shrunk, so the edges are smooth

INK = (15, 15, 15)  # the icon's black
CREAM = (240, 235, 224)  # the icon's cream
RED = (240, 4, 36)  # the icon's red


def font(size: int) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    for name in ("bahnschrift.ttf", "SegoeUI-Semibold.ttf", "DejaVuSans-Bold.ttf", "LiberationSans-Bold.ttf"):
        for folder in ("", "/usr/share/fonts/truetype/dejavu/", "/usr/share/fonts/truetype/liberation/"):
            try:
                return ImageFont.truetype(folder + name, size)
            except OSError:
                continue
    return ImageFont.load_default()


def icon(size: int) -> Image.Image:
    return Image.open(ICON).convert("RGBA").resize((size, size), Image.LANCZOS)


def finish(img: Image.Image, size: tuple[int, int], name: str) -> None:
    out = img.resize(size, Image.LANCZOS).convert("RGB")
    OUT.mkdir(parents=True, exist_ok=True)
    out.save(OUT / name, format="BMP")
    print(f"wrote {OUT / name} {size[0]}x{size[1]}")


def sidebar() -> None:
    w, h = 164 * SCALE, 314 * SCALE
    img = Image.new("RGB", (w, h), INK)
    d = ImageDraw.Draw(img)
    # A big, partly cropped copy of the icon behind everything, so the strip reads as the app and not as a box.
    ghost = icon(int(w * 1.7))
    shade = Image.new("RGBA", ghost.size, INK + (0,))
    ghost = Image.blend(ghost, shade, 0.86)
    img.paste(ghost, (int(-w * 0.55), int(h * 0.52)), ghost)
    # The icon itself, the name, and a short red rule.
    mark = icon(int(w * 0.56))
    img.paste(mark, ((w - mark.width) // 2, int(h * 0.17)), mark)
    f = font(30 * SCALE // 2)
    text = "Pyxee"
    box = d.textbbox((0, 0), text, font=f)
    d.text(((w - (box[2] - box[0])) // 2 - box[0], int(h * 0.17) + mark.height + 14 * SCALE), text, font=f, fill=CREAM)
    rule_y = int(h * 0.17) + mark.height + 14 * SCALE + (box[3] - box[1]) + 16 * SCALE
    d.rectangle(((w - 28 * SCALE) // 2, rule_y, (w + 28 * SCALE) // 2, rule_y + 2 * SCALE), fill=RED)
    finish(img, (164, 314), "sidebar.bmp")


def header() -> None:
    w, h = 150 * SCALE, 57 * SCALE
    img = Image.new("RGB", (w, h), (255, 255, 255))
    mark = icon(int(h * 0.82))
    img.paste(mark, (w - mark.width - 10 * SCALE, (h - mark.height) // 2), mark)
    finish(img, (150, 57), "header.bmp")


if __name__ == "__main__":
    sidebar()
    header()
