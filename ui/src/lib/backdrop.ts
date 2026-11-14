import { thumbCandidates } from './thumb';

/**
 * The soft colour wash behind Now Playing and the Home feature is the cover, blurred into the whole box. A
 * `filter: blur()` on a box that size makes the browser blur millions of pixels (and again whenever anything under
 * it repaints), so the blur is made once, small: the cover is shrunk to a few pixels (which blurs it), grown back in
 * doublings (which keeps it smooth) onto a canvas of `BACKDROP_PX` pixels, and the browser only stretches that.
 */
export const BACKDROP_PX = 96;
/** The picture is shrunk to this many pixels first; the fewer, the softer the wash. */
const SHRUNK_PX = 12;

/** The pictures to try for a cover this small: the small ones, never the window-sized still. */
export function backdropCandidates(src: string | null): string[] {
  return src ? thumbCandidates(src, 64, 1) : [];
}

function sizeOf(img: CanvasImageSource): [number, number] {
  const i = img as { naturalWidth?: number; naturalHeight?: number; width?: number; height?: number };
  return [i.naturalWidth || i.width || 1, i.naturalHeight || i.height || 1];
}

function scratch(px: number): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.width = c.height = px;
  return c;
}

/** Paints the blurred wash for `img` (centre square) onto `canvas`; without a picture, the flat colour a cover
 *  placeholder has. A canvas that cannot draw (no 2D context) is left as it is. */
export function paintBackdrop(canvas: HTMLCanvasElement, img: CanvasImageSource | null, hue: number): void {
  const out = canvas.getContext('2d');
  if (!out) return;
  const px = canvas.width;
  out.clearRect(0, 0, px, px);
  if (!img) {
    out.fillStyle = `hsl(${hue} 38% 32%)`;
    out.fillRect(0, 0, px, px);
    return;
  }
  const [w, h] = sizeOf(img);
  const side = Math.min(w, h);
  let size = SHRUNK_PX;
  let from: CanvasImageSource = img;
  let first = true;
  while (size < px) {
    const c = scratch(size);
    const g = c.getContext('2d');
    if (!g) return;
    g.imageSmoothingEnabled = true;
    g.imageSmoothingQuality = 'high';
    if (first) g.drawImage(from, (w - side) / 2, (h - side) / 2, side, side, 0, 0, size, size);
    else g.drawImage(from, 0, 0, size, size);
    from = c;
    first = false;
    size *= 2;
  }
  out.imageSmoothingEnabled = true;
  out.imageSmoothingQuality = 'high';
  // More colour, like the `saturate(1.6)` the blur used to carry: on this small canvas, where it costs nothing.
  // (Where the canvas has no `filter` the wash is simply less vivid.)
  out.filter = 'saturate(1.6)';
  out.drawImage(from, 0, 0, px, px);
  out.filter = 'none';
}
