/**
 * Motion for the parts that Svelte animates from script (sheets, Now Playing, snackbars). The CSS side reads the same
 * two lengths from `--dur-fast` and `--dur-base` in app.css; both fall to nothing when the system asks for less motion.
 */
export const MOTION = { fast: 120, base: 200, exit: 150 } as const;

/** True when Windows' "Show animations" is off or the page asked for reduced motion. */
export function prefersReducedMotion(): boolean {
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/** A transition length: `ms` as given, or 0 when the person asked for less motion. */
export function motionMs(ms: number): number {
  return prefersReducedMotion() ? 0 : ms;
}

/** Distance for a slide: shortened to nothing under reduced motion, so only the fade (if any) remains. */
export function motionPx(px: number): number {
  return prefersReducedMotion() ? 0 : px;
}
