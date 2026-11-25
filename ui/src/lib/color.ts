import type { Scheme, ThemeColors } from './themes';
import { isKnownMissing, isMissingStill, noteMissing, thumbCandidates } from './thumb';

/** The colours a cover is known by: its main one, one that sets itself apart from it, and sometimes a third. */
export interface CoverColors {
  a: string;
  b: string;
  /** A third colour of its own, when the cover has one (a card's looks make one up from `a` otherwise). */
  c?: string;
}

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

/** sRGB (0-255) to OKLab, the colour space where equal steps look equal. */
function toOklab(r: number, g: number, b: number): [number, number, number] {
  const lin = (v: number) => {
    const c = v / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  const [lr, lg, lb] = [lin(r), lin(g), lin(b)];
  const l = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb);
  const m = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb);
  const s = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

/** OKLab to linear sRGB (0-1; outside that range when the colour does not exist on a screen). */
function oklabToLinear(L: number, a: number, b: number): [number, number, number] {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}

/** An OKLab colour as hex; one a screen cannot show keeps its lightness and hue and gives up chroma until it fits. */
function hexFromOklab(L: number, a: number, b: number): string {
  const fits = (k: number) => oklabToLinear(L, a * k, b * k).every((v) => v >= -0.0005 && v <= 1.0005);
  let k = 1;
  if (!fits(1)) {
    let lo = 0, hi = 1;
    for (let i = 0; i < 14; i++) {
      const mid = (lo + hi) / 2;
      if (fits(mid)) lo = mid;
      else hi = mid;
    }
    k = lo;
  }
  const gamma = (v: number) => {
    const c = Math.max(0, Math.min(1, v));
    return (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055) * 255;
  };
  const [r, g, bl] = oklabToLinear(L, a * k, b * k);
  return toHex(gamma(r), gamma(g), gamma(bl));
}

/**
 * Below this OKLCH chroma a pixel is grey for our purposes: the faint warm or cool cast of a grey sleeve is below it.
 * Dark colours have less chroma for the same look (a dark navy is "bluish" at a third of it), so it scales with lightness.
 */
const minChroma = (L: number) => 0.04 * clamp(L / 0.7, 0.5, 1);
/**
 * A cover needs this much colour (the chroma of its coloured pixels, summed and divided by the number of pixels) before
 * it gets a hue; less and its theme is neutral. Measured on real covers: grey, black and white sleeves with a cast are
 * below 0.004, a black cover with a 6 % red logo is 0.011, the palest coloured cover measured was 0.025.
 */
const MIN_COLOUR = 0.008;
/** A hue must cover this much of the picture to count as a colour of the cover. */
const MIN_HUE_SHARE = 0.04;
const HUE_BINS = 36;

/** A hue the cover really has: its colour, how much of the picture it covers, and where it sits on the wheel. */
interface Hue {
  hex: string;
  share: number;
  h: number;
}

/**
 * The hues of a picture, strongest first (up to three, each at least 45 degrees from the one before), if the picture has
 * enough colour at all, and how much of the picture is coloured (0-1). Every pixel with some chroma and a lightness a theme can use votes for its hue with
 * the size of its chroma, so a broad area of a muted colour does not lose to a speck of a vivid one, and a faint cast over
 * grey does not count at all. A hue's colour is the chroma-weighted mean of the pixels around it.
 */
function hues(data: Uint8ClampedArray): { found: Hue[]; meanL: number; share: number } {
  type Bin = { mass: number; n: number; l: number; a: number; b: number };
  const bins: Bin[] = Array.from({ length: HUE_BINS }, () => ({ mass: 0, n: 0, l: 0, a: 0, b: 0 }));
  let total = 0, colour = 0, lSum = 0, lN = 0, voters = 0;
  for (let i = 0; i + 3 < data.length; i += 4) {
    if (data[i + 3]! < 128) continue;
    total++;
    const [L, a, b] = toOklab(data[i]!, data[i + 1]!, data[i + 2]!);
    if (L >= 0.15 && L <= 0.95) {
      lSum += L;
      lN++;
    }
    const c = Math.hypot(a, b);
    if (c < minChroma(L) || L < 0.08 || L > 0.96) continue;
    const bin = bins[Math.floor((((Math.atan2(b, a) * 180) / Math.PI + 360) % 360) / (360 / HUE_BINS)) % HUE_BINS]!;
    bin.mass += c; bin.n++; bin.l += L * c; bin.a += a * c; bin.b += b * c;
    colour += c;
    voters++;
  }
  const meanL = lN > 0 ? lSum / lN : 0.5;
  if (total === 0 || colour / total < MIN_COLOUR) return { found: [], meanL, share: 0 };
  const open = bins.map(() => true);
  const found: Hue[] = [];
  while (found.length < 3) {
    let best = -1, bestMass = 0;
    const window = (k: number) => {
      const w = { mass: 0, n: 0, l: 0, a: 0, b: 0 };
      for (let d = -2; d <= 2; d++) {
        const j = (k + d + HUE_BINS) % HUE_BINS;
        if (!open[j]) continue;
        const e = bins[j]!;
        w.mass += e.mass; w.n += e.n; w.l += e.l; w.a += e.a; w.b += e.b;
      }
      return w;
    };
    for (let k = 0; k < HUE_BINS; k++) {
      const w = window(k);
      if (w.mass > bestMass) { bestMass = w.mass; best = k; }
    }
    if (best < 0) break;
    const w = window(best);
    if (w.n / total < MIN_HUE_SHARE) break;
    // Every sum was weighted by the pixel's chroma, so the mean is too: the vivid pixels of the window set its colour.
    const a = w.a / w.mass, b = w.b / w.mass, L = w.l / w.mass;
    found.push({ hex: hexFromOklab(L, a, b), share: w.n / total, h: (best + 0.5) * (360 / HUE_BINS) });
    for (let d = -4; d <= 4; d++) open[(best + d + HUE_BINS) % HUE_BINS] = false;
  }
  return { found, meanL, share: voters / total };
}

/**
 * The cover's main colour, and a second one that is really in the picture: another hue, or the dark that sits beside it
 * (a yellow sleeve with black type gives yellow and near-black). Only a cover with one colour and nothing else makes
 * the second one up, as the same colour a little darker: a hue turned around the wheel is a colour the cover never had
 * (yellow turned a step is green). A third colour is given when the cover has a third hue.
 *
 * A grey, black or white cover (or one with only a faint cast, or a speck of colour) has no hue: its main colour is a
 * grey, so its theme is neutral. A hue taken from a few stray pixels would turn it pink or red.
 */
export function dominantPair(data: Uint8ClampedArray): CoverColors | null {
  const { found, meanL } = hues(data);
  let total = 0;
  for (let i = 3; i < data.length; i += 4) if (data[i]! >= 128) total++;
  if (total === 0) return null;
  return pairFrom(found, meanL, swatches(data));
}

function pairFrom(found: Hue[], meanL: number, parts: Swatch[]): CoverColors {
  const dark = (la: number) => parts.find((c) => c.share >= 0.03 && c.l < 0.36 && la - c.l >= 0.2);
  const first = found[0];
  if (!first) {
    const a = hexFromOklab(clamp(meanL, 0.4, 0.8), 0, 0);
    const [, , la] = toHsl(a);
    const b = dark(la)?.hex ?? hexFromOklab(clamp(meanL * 0.6, 0.2, 0.45), 0, 0);
    return { a, b };
  }
  const [ha, sa, la] = toHsl(first.hex);
  const second = found[1] ?? null;
  const b = second?.hex ?? dark(la)?.hex ?? fromHsl(ha, clamp(sa, 0.25, 0.9), clamp(la * 0.7, 0.12, 0.5));
  const third = found[2];
  return third ? { a: first.hex, b, c: third.hex } : { a: first.hex, b };
}

/** The cover's main colour (for the optional dynamic accent): a grey for a cover without a hue; null for no picture. */
export function dominantColor(data: Uint8ClampedArray): string | null {
  return dominantPair(data)?.a ?? null;
}

/**
 * What a cover is made of, as roles the interface can wear. The three colours are the ones `dominantPair` finds (the
 * same `a`, `b` and, when it has one, `c`), so a hue is only ever one the picture really has; the neutrals are the grey,
 * black and white that cover enough of it. `paletteFromRoles` turns the roles into colours for a scheme.
 */
export interface CoverRoles {
  /** The cover's strongest hue; a grey for a cover without one. */
  primary: string;
  /** Its next hue, else the dark beside the primary, else the primary a little darker. */
  secondary: string;
  /** A third hue, else the cover's own light neutral (white type), else the primary lighter. */
  tertiary: string;
  /** The dark and the light neutral of the cover (each at least 3 % of it, grey, black or white), if it has one. */
  dark: string | null;
  light: string | null;
  /** Whether the cover has a hue at all. One without is neutral, and so is every role. */
  hued: boolean;
  /** How much of the picture is coloured (0-1): a black sleeve with a small red logo has little, a yellow one a lot. */
  colourShare: number;
}

/** Grey, black or white for our purposes: below the chroma at which `hues` counts a pixel as coloured. */
function isNeutral(hex: string): boolean {
  const [L, a, b] = toOklab(...parseHex(hex));
  return Math.hypot(a, b) < minChroma(L);
}

/** The roles of a picture; null for one with nothing in it. */
export function coverRoles(data: Uint8ClampedArray): CoverRoles | null {
  let total = 0;
  for (let i = 3; i < data.length; i += 4) if (data[i]! >= 128) total++;
  if (total === 0) return null;
  const { found, meanL, share } = hues(data);
  const parts = swatches(data);
  const pair = pairFrom(found, meanL, parts);
  // A cover's black or white is spread over several swatches (the edges of type and drawings blend into their
  // neighbours), so its dark and light neutrals are each the average of all the neutral swatches on that side.
  const neutrals = parts.filter((c) => isNeutral(c.hex));
  const side = (on: (c: Swatch) => boolean) => {
    const group = neutrals.filter(on);
    const covered = group.reduce((n, c) => n + c.share, 0);
    if (covered < 0.03) return null;
    const mean = (at: number) => group.reduce((n, c) => n + parseHex(c.hex)[at]! * c.share, 0) / covered;
    return toHex(mean(0), mean(1), mean(2));
  };
  const dark = side((c) => c.l < 0.4);
  const light = side((c) => c.l > 0.8);
  let tertiary = pair.c ?? light;
  if (!tertiary) {
    const [h, s, l] = toHsl(pair.a);
    const lighter = clamp(l + (l > 0.72 ? -0.14 : 0.14), 0.3, 0.86);
    tertiary = found.length > 0 ? fromHsl(deepHue(h, lighter), clamp(s, 0.25, 0.9), lighter) : hexFromOklab(clamp(meanL + 0.3, 0.6, 0.95), 0, 0);
  }
  return { primary: pair.a, secondary: pair.b, tertiary, dark, light, hued: found.length > 0, colourShare: share };
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

/**
 * The centred square of a `w` x `h` picture: what the covers on screen show (they crop to a square too). YouTube's 16:9
 * stills carry the square cover art of a "Topic" upload in the middle, between two bars of a dark colour taken from the
 * art; those bars are almost half the picture and are not the cover, so they must not be read as its colour.
 */
export function centreSquare(w: number, h: number): { sx: number; sy: number; size: number } {
  const size = Math.max(1, Math.min(w, h));
  return { sx: Math.floor((w - size) / 2), sy: Math.floor((h - size) / 2), size };
}

function sizeOf(source: CanvasImageSource): [number, number] {
  if (typeof HTMLImageElement !== 'undefined' && source instanceof HTMLImageElement) return [source.naturalWidth, source.naturalHeight];
  const s = source as { width?: number; height?: number };
  return [s.width ?? 0, s.height ?? 0];
}

function pixelsOf(source: CanvasImageSource): Uint8ClampedArray | null {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 32;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return null;
  const [w, h] = sizeOf(source);
  if (w > 0 && h > 0) {
    const { sx, sy, size } = centreSquare(w, h);
    ctx.drawImage(source, sx, sy, size, size, 0, 0, 32, 32);
  } else {
    ctx.drawImage(source, 0, 0, 32, 32);
  }
  return ctx.getImageData(0, 0, 32, 32).data;
}

const sampleRoles = (source: CanvasImageSource): CoverRoles | null => {
  const px = pixelsOf(source);
  return px ? coverRoles(px) : null;
};
const samplePair = (source: CanvasImageSource): CoverColors | null => {
  const px = pixelsOf(source);
  return px ? dominantPair(px) : null;
};

const rolesCache = new Map<string, CoverRoles>();
const COLOR_CACHE_MAX = 64;

/**
 * Loads a cover and returns its roles (its main colour is `primary`), or null if it cannot be had. It asks for the same
 * small images the covers on screen use (https, the sized or still variant) and moves on to the next one when a picture
 * is missing. The page can only read an image's pixels when the image host allows it; when it does not, `fetchBytes`
 * (the app fetching the picture itself) is used instead. A cover's roles are remembered, so a song that comes round
 * again costs nothing.
 */
export async function coverRolesFromImage(url: string, fetchBytes?: (url: string) => Promise<ArrayBuffer>): Promise<CoverRoles | null> {
  const known = rolesCache.get(url);
  if (known) return known;
  const found = await readColors(url, fetchBytes, sampleRoles);
  if (found) {
    if (rolesCache.size >= COLOR_CACHE_MAX) rolesCache.delete(rolesCache.keys().next().value as string);
    rolesCache.set(url, found);
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

/** The surfaces, lines and text of a palette: the hue `h` at strength `n`, deep in the dark scheme, pale in the light one. */
function layers(h: number, n: number, scheme: Scheme) {
  // Yellow turned dark would be olive green: every colour below takes the hue it needs at its own lightness.
  const at = (sat: number, light: number) => fromHsl(deepHue(h, light), sat, light);
  if (scheme === 'dark') {
    const surface2 = at(n, 0.21);
    return {
      bg: at(n, 0.115),
      surface: at(n, 0.155),
      surface2,
      line: at(n * 0.9, 0.3),
      fg: at(n * 0.4, 0.95),
      muted: ensureContrastOn(at(n * 0.8, 0.7), surface2, 'dark', 4.5),
    };
  }
  const surface2 = at(n, 0.855);
  return {
    bg: at(n, 0.935),
    surface: at(n, 0.9),
    surface2,
    line: at(n * 0.9, 0.72),
    fg: at(n * 0.6, 0.1),
    muted: ensureContrastOn(at(n * 0.7, 0.34), surface2, 'light', 4.5),
  };
}

/**
 * A whole theme from one colour (the cover's): background, panels, lines, text and accent all take its hue, and the
 * surfaces are visibly tinted by it. Dark or light follows `scheme`: a deep shade of the colour in dark mode, a pale
 * one in light mode. The text and the accent are nudged until they read on the background and on the raised panels,
 * as the built-in themes do.
 */
export function paletteFromArt(art: string, scheme: Scheme): ThemeColors {
  const [h, s, l] = toHsl(art);
  const at = (sat: number, light: number) => fromHsl(deepHue(h, light), sat, light);
  const layer = layers(h, surfaceTint(s, scheme === 'dark' ? 0.6 : 0.5), scheme);
  const accent =
    scheme === 'dark'
      ? ensureContrastOn(at(accentSat(s), clamp(l, 0.58, 0.74)), layer.surface2, 'dark', 3)
      : ensureContrastOn(at(accentSat(s), clamp(l, 0.28, 0.4)), layer.surface2, 'light', 3);
  return { ...layer, accent };
}

/** A theme made of a cover's roles: the palette as `paletteFromArt` makes it, and the two colours the rest of the interface wears. */
export interface RolePalette {
  /** Surfaces and text from the cover's neutrals, the accent from its primary. */
  colors: ThemeColors;
  /** The secondary: the second stop of gradients, fills such as the seek bar. */
  accent2: string;
  /** The tertiary: small highlights. */
  accent3: string;
}

/**
 * How a role is worn: the band its hsl lightness is held in, by scheme. A coloured cover's roles stand apart by hue (the
 * primary's band is `paletteFromArt`'s); a grey one's can only stand apart by lightness, so its bands do not overlap.
 */
const WEAR = {
  dark: {
    hued: { primary: [0.58, 0.74], secondary: [0.46, 0.62], tertiary: [0.74, 0.88] },
    grey: { primary: [0.7, 0.76], secondary: [0.5, 0.54], tertiary: [0.88, 0.94] },
  },
  light: {
    hued: { primary: [0.28, 0.4], secondary: [0.18, 0.3], tertiary: [0.34, 0.42] },
    grey: { primary: [0.38, 0.44], secondary: [0.12, 0.18], tertiary: [0.24, 0.3] },
  },
} as const;

/**
 * The palette for a cover's roles. The surfaces and text are the cover's neutral side: they take the primary's hue only
 * as far as the picture is coloured (a grey cover is exactly grey, a black one with a small red logo hardly tinted, a
 * yellow one clearly warm). The three accents are the three roles, each brought into its lightness band, then nudged
 * until it holds 3:1 against the raised panels (slider fills, focus rings, icons) and until the text on it reads at
 * 4.5:1: dark text in the dark scheme, white in the light one, so one on-colour serves every role and every gradient
 * between two of them. A role that cannot be worn in this scheme (a black one on a dark page) is the primary's hue
 * darker or deeper instead: a hue the cover has, never one turned around the wheel.
 */
export function paletteFromRoles(roles: CoverRoles, scheme: Scheme): RolePalette {
  const dark = scheme === 'dark';
  const [h, s] = toHsl(roles.primary);
  // Tinted as deeply as a one-colour theme, the surfaces of a yellow cover turn to olive mud and bury the accents: they
  // take a little over half of it, and less as less of the picture is coloured.
  const strength = roles.hued ? clamp(roles.colourShare / 0.3, 0.3, 1) * 0.55 : 1;
  const layer = layers(h, surfaceTint(s, dark ? 0.6 : 0.5) * strength, scheme);
  const textOn = dark ? '#0f0f0f' : '#ffffff';
  const wear = (hex: string, band: readonly [number, number]) => {
    const [hh, ss, ll] = toHsl(hex);
    const light = clamp(ll, band[0], band[1]);
    // A neutral is exactly grey: the cast of a sleeve's black or white is not a colour.
    const worn = fromHsl(deepHue(hh, light), isNeutral(hex) ? 0 : accentSat(ss), light);
    return ensureContrastOn(ensureContrastOn(worn, layer.surface2, scheme, 3), textOn, scheme, 4.5);
  };
  const [hp, sp, lp] = toHsl(roles.primary);
  const shade = (f: number) => fromHsl(deepHue(hp, lp * f), clamp(sp, 0.25, 0.9), lp * f);
  const unreadable = (hex: string) => roles.hued && isNeutral(hex) && (dark ? luminance(hex) < 0.2 : luminance(hex) > 0.5);
  const bands = WEAR[scheme][roles.hued ? 'hued' : 'grey'];
  let primary = wear(roles.primary, bands.primary);
  let accent2 = wear(unreadable(roles.secondary) ? shade(0.75) : roles.secondary, bands.secondary);
  let accent3 = wear(unreadable(roles.tertiary) ? shade(dark ? 1.15 : 0.6) : roles.tertiary, bands.tertiary);
  // The chosen row is the raised panel tinted by the primary, so the primary's own bar and title, and the tertiary's
  // playing mark, must hold there too: it is the hardest ground they sit on. (The secondary only tints it as a sweep.)
  const row = () => mixHex(layer.surface2, primary, 0.16);
  const further = dark ? '#ffffff' : '#000000';
  for (let i = 0; i < 3; i++) primary = ensureContrastOn(primary, row(), scheme, 3);
  // The title of the chosen row is the primary mixed into the text colour (accent-ink): it reads at 4.5:1 there and on the page.
  const inkReads = () => [row(), layer.bg].every((g) => contrast(mixHex(layer.fg, primary, 0.75), g) >= 4.5);
  for (let i = 0; i < 8 && !inkReads(); i++) primary = mixHex(primary, further, 0.06);
  accent3 = ensureContrastOn(accent3, row(), scheme, 3);
  // The play button and the chosen tab run from the primary to the secondary under one text colour: it must read all along.
  const onPrimary = readableOn(primary);
  const reads = (end: string) => Array.from({ length: 9 }, (_, i) => i / 8).every((t) => contrast(onPrimary, mixHex(primary, end, t)) >= 4.5);
  for (let i = 0; i < 8 && !reads(accent2); i++) accent2 = mixHex(accent2, primary, 0.2);
  if (!reads(accent2)) accent2 = primary;
  return { colors: { ...layer, accent: primary }, accent2, accent3 };
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
