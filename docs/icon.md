# Icon and logo

The mark is your red/black disc, kept as drawn: `assets/logo/logo.svg` is a vector trace of the supplied PNG's
silhouette (potrace, IoU 0.999 against the original) with the ring, cream disc, centre dot and stem rebuilt as exact
geometry. Colours: red `#F00524`, black `#0F0E0F`, cream `#F2EDE2`.

- `assets/logo/tray.svg`: simplified for 16-24 px (thicker stem, larger cream disc, no centre dot).
- `python3 assets/logo/make-icons.py` regenerates everything below (needs `rsvg-convert` and Pillow):
  `crates/app/icons/icon.ico` (16, 20, 24, 32, 40, 48, 64, 128 as 32-bit BMP frames, 256 as PNG; up to 24 px use the tray variant), `icon.png` (512),
  `32x32.png`, `128x128.png`, `128x128@2x.png`, `tray.ico` + `tray-16/24/32.png` (for the system tray in WP4) and
  `assets/logo/png/logo-1024.png`.
- Side rail: `ui/src/components/Logo.svelte` (same artwork inline; its colours do not follow the theme).
- Accent: Dark and Light use the logo red. The UI accent is nudged to `#ec0422` (Dark) and `#d4041f` (Light) because
  white text on the exact `#F00524` only reaches 4.42:1 and the readability contract asks for 4.5:1.
