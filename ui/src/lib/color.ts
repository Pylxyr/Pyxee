import type { ThemeColors } from './themes';
import { isKnownMissing, isMissingStill, noteMissing, thumbCandidates } from './thumb';
type Bucket = { w: number; r: number; g: number; b: number };

/** The colour buckets of the pixels whose lightness is within [lo, hi], heaviest first. */
function buckets(data: Uint8ClampedArray, lo: number, hi: number, greyWeight: number): string[] {
  const found = new Map<number, Bucket>();
  for (let i = 0; i + 3 < data.length; i += 4) {
    const r = data[i]!, g = data[i + 1]!, b = data[i + 2]!, a = data[i + 3]!;
    if (a < 128) continue;
    const max = Math.max(r, g, b), min = Math.min(r, g, b);
    const light = (max + min) / 510;
    if (light < lo || light > hi) continue;
    const sat = max === min ? 0 : (max - min) / (255 - Math.abs(max + min - 255));
    const key = ((r >> 5) << 6) | ((g >> 5) << 3) | (b >> 5);
    const weight = greyWeight + sat; // prefer colourful pixels over grey ones
    const e = found.get(key) ?? { w: 0, r: 0, g: 0, b: 0 };
    e.w += weight; e.r += r * weight; e.g += g * weight; e.b += b * weight;
    found.set(key, e);
  }
  return [...found.values()].sort((x, y) => y.w - x.w).map((e) => toHex(e.r / e.w, e.g / e.w, e.b / e.w));
}

/** The heaviest colour bucket of the pixels whose lightness is within [lo, hi]; null when none qualifies. */
function heaviest(data: Uint8ClampedArray, lo: number, hi: number, greyWeight: number): string | null {
  return buckets(data, lo, hi, greyWeight)[0] ?? null;
}

/**
 * Picks a vivid representative colour from RGBA pixel data (for the optional dynamic accent). Near-black and
 * near-white pixels are poor accents, so they are left out first; a cover made only of them (a dark or a white
 * sleeve) would then have no colour at all, so it is read again with those allowed rather than giving up.
 */
export function dominantColor(data: Uint8ClampedArray): string | null {
  return heaviest(data, 0.12, 0.9, 0.15) ?? heaviest(data, 0.03, 0.97, 0.05);
}

/** The colours a cover is known by: its main one, one that sets itself apart from it, and sometimes a third. */
export interface CoverColors {
  a: string;
  b: string;
  /** A third colour of its own, when the cover has one (a card's looks make one up from `a` otherwise). */
  c?: string;
}

/** How far apart two hues are, in degrees (0-180). */
const hueGap = (x: number, y: number) => {
  const d = Math.abs(x - y) % 360;
  return d > 180 ? 360 - d : d;
};

/** A colour of the picture and how much of the picture it covers (0-1), grey, black and white included. */
interface Swatch {
  hex: string;
  share: number;
  h: number;
  s: number;
  l: number;
}

/** What the picture is made of, by how much of it each colour covers, largest first. */
function swatches(data: Uint8ClampedArray): Swatch[] {
  const found = new Map<number, { n: number; r: number; g: number; b: number }>();
  let total = 0;
  for (let i = 0; i + 3 < data.length; i += 4) {
    if (data[i + 3]! < 128) continue;
    const r = data[i]!, g = data[i + 1]!, b = data[i + 2]!;
    const key = ((r >> 5) << 6) | ((g >> 5) << 3) | (b >> 5);
    const e = found.get(key) ?? { n: 0, r: 0, g: 0, b: 0 };
    e.n++; e.r += r; e.g += g; e.b += b;
    found.set(key, e);
    total++;
  }
  return [...found.values()]
    .sort((x, y) => y.n - x.n)
    .map((e) => {
      const hex = toHex(e.r / e.n, e.g / e.n, e.b / e.n);
      const [h, s, l] = toHsl(hex);
      return { hex, share: e.n / total, h, s, l };
    });
}

/** Colour that is not grey, black or white: something a hue can be told from. */
const chromatic = (c: Swatch) => c.s > 0.15 && c.l >= 0.1 && c.l <= 0.93;

/**
 * The cover's main colour, and a second one that is really in the picture: another hue, or the dark that sits beside it
 * (a yellow sleeve with black type gives yellow and near-black). Only a cover with one colour and nothing else makes
 * the second one up, as the same colour a little darker: a hue turned around the wheel is a colour the cover never had
 * (yellow turned a step is green). A third colour is given when the cover has a third hue.
 */
export function dominantPair(data: Uint8ClampedArray): CoverColors | null {
  const list = buckets(data, 0.12, 0.9, 0.15);
  const all = list.length > 0 ? list : buckets(data, 0.03, 0.97, 0.05);
  const a = all[0];
  if (!a) return null;
  const [ha, sa, la] = toHsl(a);
  const parts = swatches(data);
  // A picture is read small, so black type on a cover is a dark grey by then (not black): "dark" reaches up to 0.36.
  const apart = (c: Swatch) => c.share >= 0.03 && ((chromatic(c) && sa > 0.12 && hueGap(c.h, ha) >= 35) || (c.l < 0.36 && la - c.l >= 0.2));
  const second = parts.find(apart);
  const b = second?.hex ?? fromHsl(ha, clamp(sa, 0.25, 0.9), clamp(la * 0.7, 0.12, 0.5));
  const third = parts.find((c) => c !== second && c.share >= 0.03 && chromatic(c) && hueGap(c.h, ha) >= 35 && (!second || !chromatic(second) || hueGap(c.h, second.h) >= 35));
  return third ? { a, b, c: third.hex } : { a, b };
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

function pixelsOf(source: CanvasImageSource): Uint8ClampedArray | null {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 32;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return null;
  ctx.drawImage(source, 0, 0, 32, 32);
  return ctx.getImageData(0, 0, 32, 32).data;
}

const sampleColor = (source: CanvasImageSource): string | null => {
  const px = pixelsOf(source);
  return px ? dominantColor(px) : null;
};
const samplePair = (source: CanvasImageSource): CoverColors | null => {
  const px = pixelsOf(source);
  return px ? dominantPair(px) : null;
};

const colorCache = new Map<string, string>();
const COLOR_CACHE_MAX = 64;

/**
 * Loads a cover and returns its dominant colour, or null if it cannot be had. It asks for the same small images the
 * covers on screen use (https, the sized or still variant) and moves on to the next one when a picture is missing.
 * The page can only read an image's pixels when the image host allows it; when it does not, `fetchBytes` (the app
 * fetching the picture itself) is used instead. A cover's colour is remembered, so a song that comes round again costs nothing.
 */
export async function accentFromImage(url: string, fetchBytes?: (url: string) => Promise<ArrayBuffer>): Promise<string | null> {
  const known = colorCache.get(url);
  if (known) return known;
  const found = await readColors(url, fetchBytes, sampleColor);
  if (found) {
    if (colorCache.size >= COLOR_CACHE_MAX) colorCache.delete(colorCache.keys().next().value as string);
    colorCache.set(url, found);
  }
  return found;
}

const pairCache = new Map<string, CoverColors>();
const pending = new Map<string, Promise<CoverColors | null>>();

/**
 * A cover's two colours (for the "Tune in" cards and the feature card's looks), read the way `accentFromImage` reads
 * one. Remembered per cover; several cards asking for the same cover at once share one read.
 */
export function coverColors(url: string, fetchBytes?: (url: string) => Promise<ArrayBuffer>): Promise<CoverColors | null> {
  const known = pairCache.get(url);
  if (known) return Promise.resolve(known);
  const running = pending.get(url);
  if (running) return running;
  const job = readColors(url, fetchBytes, samplePair)
    .then((found) => {
      if (found) {
        if (pairCache.size >= COLOR_CACHE_MAX * 2) pairCache.delete(pairCache.keys().next().value as string);
        pairCache.set(url, found);
      }
      return found;
    })
    .finally(() => pending.delete(url));
  pending.set(url, job);
  return job;
}

/** The colours of a cover already read (no waiting), for something that is drawn again. */
export const knownCoverColors = (url: string): CoverColors | undefined => pairCache.get(url);

async function readColors<T>(url: string, fetchBytes: ((url: string) => Promise<ArrayBuffer>) | undefined, sample: (source: CanvasImageSource) => T | null): Promise<T | null> {
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
      const found = sample(img);
      if (found) return found;
      continue;
    } catch {
      /* not readable from the page (no CORS answer) or not there: ask the app for the picture */
    }
    if (!fetchBytes) continue;
    try {
      const bitmap = await createImageBitmap(new Blob([await fetchBytes(candidate)]));
      try {
        if (isMissingStill(candidate, bitmap.width, bitmap.height)) {
          noteMissing(candidate);
          continue;
        }
        const found = sample(bitmap);
        if (found) return found;
        continue;
      } finally {
        bitmap.close();
      }
    } catch {
      /* this one is not there either: try the next */
    }
  }
  return null;
}

function toHsl(hex: string): [number, number, number] {
  const [r, g, b] = parseHex(hex).map((v) => v / 255) as [number, number, number];
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = d / (1 - Math.abs(2 * l - 1));
  const h = max === r ? ((g - b) / d + (g < b ? 6 : 0)) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h * 60, s, l];
}

function fromHsl(h: number, s: number, l: number): string {
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    return l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
  };
  return toHex(f(0) * 255, f(8) * 255, f(4) * 255);
}

const clamp = (n: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, n));

/**
 * A yellow made dark is not a dark yellow: at a low lightness the hue of yellow (60 degrees) reads as olive, a green. A
 * colour of that hue is turned toward amber the darker it gets, so it stays the gold or the bronze the cover shows.
 */
export function deepHue(h: number, l: number): number {
  const weight = clamp((74 - h) / 10, 0, 1) * (h >= 48 ? 1 : 0);
  if (weight === 0) return h;
  return h - clamp((0.56 - l) / 0.26, 0, 1) * 0.85 * weight * (h - 40);
}

/** `a` moved `t` (0-1) of the way to `b`, in sRGB. */
export function mixHex(a: string, b: string, t: number): string {
  const [ar, ag, ab] = parseHex(a);
  const [br, bg, bb] = parseHex(b);
  return toHex(ar + (br - ar) * t, ag + (bg - ag) * t, ab + (bb - ab) * t);
}

/** How much of a cover's colour the surfaces take: a grey cover stays grey, a coloured one is clearly tinted. */
const surfaceTint = (s: number, ceiling: number) => (s < 0.08 ? s : clamp(s * 0.8, 0.22, ceiling));

/**
 * How saturated the accent is for a cover of saturation `s`: strong for a colourful cover, and only as much as the cover
 * has for a grey one. A grey, black or white sleeve has no hue (it reads as 0 degrees, red), so lifting it to a strong
 * colour made a grey cover a red-orange theme.
 */
const accentSat = (s: number) => (s < 0.3 ? s * 2 : clamp(s, 0.6, 0.95));

/**
 * A whole theme from one colour (the cover's): background, panels, lines, text and accent all take its hue, and the
 * surfaces are visibly tinted by it. Dark or light follows `scheme`: a deep shade of the colour in dark mode, a pale
 * one in light mode. The text and the accent are nudged until they read on the background and on the raised panels,
 * as the built-in themes do.
 */
export function paletteFromArt(art: string, scheme: 'dark' | 'light'): ThemeColors {
  const [h, s, l] = toHsl(art);
  // Yellow turned dark would be olive green: every colour below takes the hue it needs at its own lightness.
  const at = (sat: number, light: number) => fromHsl(deepHue(h, light), sat, light);
  if (scheme === 'dark') {
    const n = surfaceTint(s, 0.6);
    const bg = at(n, 0.115);
    const surface = at(n, 0.155);
    const surface2 = at(n, 0.21);
    return {
      bg,
      surface,
      surface2,
      line: at(n * 0.9, 0.3),
      fg: at(n * 0.4, 0.95),
      muted: ensureContrastOn(at(n * 0.8, 0.7), surface2, 'dark', 4.5),
      accent: ensureContrastOn(at(accentSat(s), clamp(l, 0.58, 0.74)), surface2, 'dark', 3),
    };
  }
  const n = surfaceTint(s, 0.5);
  const bg = at(n, 0.935);
  const surface = at(n, 0.9);
  const surface2 = at(n, 0.855);
  return {
    bg,
    surface,
    surface2,
    line: at(n * 0.9, 0.72),
    fg: at(n * 0.6, 0.1),
    muted: ensureContrastOn(at(n * 0.7, 0.34), surface2, 'light', 4.5),
    accent: ensureContrastOn(at(accentSat(s), clamp(l, 0.28, 0.4)), surface2, 'light', 3),
  };
}

/** `hex` with its saturation and lightness held inside the given ranges (a grey one stays grey). */
function tone(hex: string, sat: [number, number], light: [number, number]): string {
  const [h, s, l] = toHsl(hex);
  const lightness = clamp(l, light[0], light[1]);
  return fromHsl(deepHue(h, lightness), s < 0.08 ? s : clamp(s, sat[0], sat[1]), lightness);
}

/** The two colours of a "Tune in" card's gradient: the cover's, deepened until white words read on both (WCAG 4.5:1). */
export function cardTones(c: CoverColors): CoverColors {
  return {
    a: ensureContrastOn(tone(c.a, [0.4, 0.85], [0.26, 0.42]), '#ffffff', 'light', 4.5),
    b: ensureContrastOn(tone(c.b, [0.35, 0.8], [0.14, 0.3]), '#ffffff', 'light', 4.5),
  };
}

/**
 * The three colours a feature card's looks are painted with: lively, the third the cover's own when it has one, else a
 * lighter turn of the first (a few degrees, away from green, so yellow stays gold and does not drift into lime).
 */
export function heroTones(c: CoverColors): { a: string; b: string; c: string } {
  const a = tone(c.a, [0.45, 0.9], [0.4, 0.62]);
  const b = tone(c.b, [0.4, 0.85], [0.35, 0.58]);
  if (c.c) return { a, b, c: tone(c.c, [0.4, 0.85], [0.4, 0.58]) };
  const [h, s, l] = toHsl(a);
  const turned = (h + (h >= 45 && h < 200 ? -14 : 14) + 360) % 360;
  const lightness = clamp(l + 0.08, 0.4, 0.66);
  return { a, b, c: fromHsl(deepHue(turned, lightness), s < 0.08 ? s : clamp(s, 0.4, 0.85), lightness) };
}

/** What the native window frame wears: its border, and (with the Windows title bar) the bar and its text. */
export interface WindowTint {
  border: string;
  caption: string;
  text: string;
}

export function windowTint(c: ThemeColors): WindowTint {
  return { border: mixHex(c.line, c.accent, 0.55), caption: c.bg, text: c.fg };
}
