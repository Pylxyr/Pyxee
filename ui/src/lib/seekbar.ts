/**
 * The maths behind the seek bar's smoother modes (see `components/Seek.svelte`). Kept apart from the component so it
 * can be tested without a page: nothing here touches the DOM or a clock.
 */

/** How the seek bar fills in. `step` is the plain once-a-second move; the others are for comparing on a real PC. */
export type SeekMode = 'step' | 'pixel' | 'glide' | 'steps';
export const SEEK_MODES: SeekMode[] = ['step', 'pixel', 'glide', 'steps'];

/** What a person (or the `.env` file) typed for the setting; anything unknown means the plain 1 Hz step. */
export function parseSeekMode(value: string | null | undefined): SeekMode {
  const v = (value ?? '').trim().toLowerCase();
  return (SEEK_MODES as string[]).includes(v) ? (v as SeekMode) : 'step';
}

/** Steps a second for the `steps` mode: a few, since the bar moves about 2 px a second and each step is then under a pixel. */
export const STEPS_PER_SECOND = 4;

/** Whole device pixels in a CSS width. */
export function devicePixels(cssWidth: number, ratio: number): number {
  return Math.max(0, Math.round(cssWidth * (ratio > 0 ? ratio : 1)));
}

/** The whole pixel the fill reaches at `positionMs`, out of `widthPx`. */
export function pixelOf(positionMs: number, durationMs: number, widthPx: number): number {
  if (!(durationMs > 0) || !(widthPx > 0)) return 0;
  const p = Math.min(1, Math.max(0, positionMs / durationMs));
  return Math.min(widthPx, Math.floor(p * widthPx));
}

/** The fill's width, in percent, snapped down to a whole pixel (what the screen can show anyway). */
export function pixelPct(positionMs: number, durationMs: number, widthPx: number): number {
  return widthPx > 0 ? (pixelOf(positionMs, durationMs, widthPx) / widthPx) * 100 : 0;
}

/**
 * Milliseconds until the fill needs its next pixel, or null when there is nothing to wait for (no length, no width, or
 * the bar is already full). Never less than 1 ms, so a timer set from it cannot spin.
 */
export function nextPixelDelayMs(positionMs: number, durationMs: number, widthPx: number): number | null {
  if (!(durationMs > 0) || !(widthPx > 0) || positionMs >= durationMs) return null;
  const next = pixelOf(positionMs, durationMs, widthPx) + 1;
  if (next > widthPx) return null;
  return Math.max(1, Math.ceil((next * durationMs) / widthPx - positionMs));
}

/** How many steps the `steps` mode divides the rest of the song into: a few a second, but never more than the pixels left to fill (so a step is at least a pixel and the bar never idles). */
export function stepsFor(remainingMs: number, remainingPx: number): number {
  return Math.max(1, Math.min(Math.ceil((remainingMs / 1000) * STEPS_PER_SECOND), Math.max(1, remainingPx)));
}

/** Where a running fill animation says the song is: where it started plus how long it has run. */
export function animatedPositionMs(startMs: number, runMs: number, durationMs: number): number {
  return Math.min(durationMs, startMs + Math.max(0, runMs));
}

/** The drift, in ms, above which the animation is restarted from the reported position. */
export const DRIFT_RESYNC_MS = 300;
