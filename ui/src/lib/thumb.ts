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
    // A square crop of a 16:9 still is as tall as the still: 180 px for mqdefault, 720 for the others.
    if (size <= 180) return [`${base}mqdefault${ext}`];
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
