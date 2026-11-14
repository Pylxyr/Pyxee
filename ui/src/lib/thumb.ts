/**
 * Picks the image to request for a cover, for a box `px` CSS pixels wide on a screen with `dpr`.
 *
 * YouTube's 4:3 stills (default, hqdefault, sddefault) have black bars baked in for widescreen
 * videos, so a centre-crop cannot remove them. The 16:9 stills (mqdefault 320x180, hq720 and
 * maxresdefault) have no bars: cropped to a square they show the picture, or the square cover
 * art that "Topic" uploads centre in the frame. Cover URLs from Google's image hosts accept a
 * size parameter, so we ask for the size we show instead of whatever search happened to return.
 *
 * Returns the URLs to try in order (a missing maxresdefault is a 404, then the next is used).
 */
const YT_STILL = /^(https?:\/\/i\d?\.ytimg\.com\/vi(?:_webp)?\/[^/]+\/)(?:default|mqdefault|hqdefault|sddefault|hq720|maxresdefault)(\.(?:jpg|webp))(\?.*)?$/;
const GOOGLE_HOST = /^https?:\/\/(?:lh\d\.googleusercontent\.com|yt\d\.ggpht\.com|yt\d\.googleusercontent\.com)\//;

/** Rounded up to a multiple of 64 (fewer distinct URLs to cache), at most 1200. */
export function deviceSize(px: number, dpr: number): number {
  const wanted = Math.max(1, Math.ceil(px * Math.max(1, dpr)));
  return Math.min(1200, Math.ceil(wanted / 64) * 64);
}

export function thumbCandidates(src: string, px: number, dpr = 1): string[] {
  const secure = src.replace(/^http:\/\//, 'https://');
  const size = deviceSize(px, dpr);

  const still = YT_STILL.exec(secure);
  if (still) {
    const [, base, ext] = still;
    // A square crop of a 16:9 still is as tall as the still: 180 px for mqdefault, 720 for the others. A 720 px picture
    // shown in a much smaller box is shrunk by a large factor and looks harsh and jagged, so the small still is kept
    // up to a box a little larger than it (a mild upscale is soft, not harsh).
    if (size <= 256) return [`${base}mqdefault${ext}`];
    return [`${base}maxresdefault${ext}`, `${base}hq720${ext}`, `${base}mqdefault${ext}`];
  }

  if (GOOGLE_HOST.test(secure)) {
    const sized = secure
      .replace(/=w\d+-h\d+/, `=w${size}-h${size}`)
      .replace(/=s\d+/, `=s${size}`);
    return sized === secure ? [secure] : [sized, secure];
  }

  return [secure];
}

const YT_HOST = /^https?:\/\/i\d?\.ytimg\.com\//;

/**
 * YouTube answers a still that does not exist (no maxresdefault or hq720 for many videos, or a deleted video) with a
 * small grey "no picture" image (120x90) and a 404 status. An <img> shows that body like any other picture and fires
 * `load`, not `error`, so it has to be recognised by its size. Every real still we ask for is larger than this.
 */
export function isMissingStill(url: string, width: number, height: number): boolean {
  return YT_HOST.test(url) && width > 0 && width <= 120 && height > 0 && height <= 90;
}

/** Stills already found to be missing, so another row showing the same song does not request them again. */
const missing = new Set<string>();

export function noteMissing(url: string): void {
  if (missing.size >= 2000) missing.clear();
  missing.add(url);
}

export function isKnownMissing(url: string): boolean {
  return missing.has(url);
}
