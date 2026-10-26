import { isKnownMissing, isMissingStill, noteMissing, thumbCandidates } from './thumb';
/** Picks a vivid representative colour from RGBA pixel data (for the optional dynamic accent). */
export function dominantColor(data: Uint8ClampedArray): string | null {
  const buckets = new Map<number, { w: number; r: number; g: number; b: number }>();
  for (let i = 0; i + 3 < data.length; i += 4) {
    const r = data[i]!, g = data[i + 1]!, b = data[i + 2]!, a = data[i + 3]!;
    if (a < 128) continue;
    const max = Math.max(r, g, b), min = Math.min(r, g, b);
    const light = (max + min) / 510;
    if (light < 0.12 || light > 0.9) continue; // near-black / near-white are poor accents
    const sat = max === min ? 0 : (max - min) / (255 - Math.abs(max + min - 255));
    const key = ((r >> 5) << 6) | ((g >> 5) << 3) | (b >> 5);
    const weight = 0.15 + sat; // prefer colourful pixels over grey ones
    const e = buckets.get(key) ?? { w: 0, r: 0, g: 0, b: 0 };
    e.w += weight; e.r += r * weight; e.g += g * weight; e.b += b * weight;
    buckets.set(key, e);
  }
  let best: { w: number; r: number; g: number; b: number } | null = null;
  for (const e of buckets.values()) if (!best || e.w > best.w) best = e;
  return best ? toHex(best.r / best.w, best.g / best.w, best.b / best.w) : null;
}

export function toHex(r: number, g: number, b: number): string {
  const c = (n: number) => Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, '0');
  return `#${c(r)}${c(g)}${c(b)}`;
}

function parseHex(hex: string): [number, number, number] {
  const h = hex.replace('#', '');
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}

/** WCAG relative luminance. */
export function luminance(hex: string): number {
  const [r, g, b] = parseHex(hex).map((v) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

/** Nudges `hex` toward white (dark scheme) or black (light scheme) until it reads against `bg`. */
export function ensureContrastOn(hex: string, bg: string, scheme: 'dark' | 'light', min = 3): string {
  const target = scheme === 'dark' ? 255 : 0;
  let [r, g, b] = parseHex(hex);
  for (let i = 0; i < 20 && contrast(toHex(r, g, b), bg) < min; i++) {
    r += (target - r) * 0.12; g += (target - g) * 0.12; b += (target - b) * 0.12;
  }
  return toHex(r, g, b);
}

/** Same, against the default dark (#0f0f0f) or light (#ffffff) background. */
export function ensureContrast(hex: string, theme: 'dark' | 'light', min = 3): string {
  return ensureContrastOn(hex, theme === 'dark' ? '#0f0f0f' : '#ffffff', theme, min);
}

/** The text colour (near-black or white) that reads better on `bg`. */
export function readableOn(bg: string): string {
  return contrast('#ffffff', bg) >= contrast('#0f0f0f', bg) ? '#ffffff' : '#0f0f0f';
}

export const HEX = /^#[0-9a-f]{6}$/i;

/** `#abc` and `ABCDEF` become `#aabbcc` / `#abcdef`; anything else is null. */
export function normalizeHex(v: string): string | null {
  let h = v.trim().replace(/^#/, '').toLowerCase();
  if (/^[0-9a-f]{3}$/.test(h)) h = [...h].map((c) => c + c).join('');
  return /^[0-9a-f]{6}$/.test(h) ? `#${h}` : null;
}

function sampleColor(source: CanvasImageSource): string | null {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 32;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return null;
  ctx.drawImage(source, 0, 0, 32, 32);
  return dominantColor(ctx.getImageData(0, 0, 32, 32).data);
}

/**
 * Loads a cover and returns its dominant colour, or null if it cannot be had. It asks for the same small images the
 * covers on screen use (https, the sized or still variant) and moves on to the next one when a picture is missing.
 * The page can only read an image's pixels when the image host allows it; when it does not, `fetchBytes` (the app
 * fetching the picture itself) is used instead.
 */
export async function accentFromImage(url: string, fetchBytes?: (url: string) => Promise<number[]>): Promise<string | null> {
  for (const candidate of thumbCandidates(url, 64).filter((u) => !isKnownMissing(u))) {
    try {
      const img = new Image();
      img.crossOrigin = 'anonymous';
      img.src = candidate;
      await img.decode();
      if (isMissingStill(candidate, img.naturalWidth, img.naturalHeight)) {
        noteMissing(candidate);
        continue;
      }
      return sampleColor(img);
    } catch {
      /* not readable from the page (no CORS answer) or not there: ask the app for the picture */
    }
    if (!fetchBytes) continue;
    try {
      const bitmap = await createImageBitmap(new Blob([new Uint8Array(await fetchBytes(candidate))]));
      try {
        if (isMissingStill(candidate, bitmap.width, bitmap.height)) {
          noteMissing(candidate);
          continue;
        }
        return sampleColor(bitmap);
      } finally {
        bitmap.close();
      }
    } catch {
      /* this one is not there either: try the next */
    }
  }
  return null;
}
