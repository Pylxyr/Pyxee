import { tick } from 'svelte';

/**
 * Motion for the parts that Svelte animates from script (sheets, Now Playing, snackbars). The CSS side reads the same
 * two lengths from `--dur-fast` and `--dur-base` in app.css; both fall to nothing when the system asks for less motion.
 */
export const MOTION = { fast: 120, base: 200, exit: 150, slow: 360 } as const;

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

/**
 * Springs for the parts Svelte animates from script: they overshoot a little and settle, like the CSS `--spring` and
 * `--spring-pop` (the same two curves). `springOut` is soft (about 9% past the end), `springPop` is livelier (about 14%).
 */
export const springOut = (t: number): number => (t <= 0 ? 0 : t >= 1 ? 1 : 1 - Math.exp(-8 * t) * Math.cos(9 * t));
export const springPop = (t: number): number => (t <= 0 ? 0 : t >= 1 ? 1 : 1 - Math.exp(-7 * t) * Math.cos(10 * t));

type ViewTransitionLike = { ready: Promise<unknown>; finished: Promise<unknown>; updateCallbackDone: Promise<unknown> };

/**
 * Runs `update` (a state change that re-renders) inside a view transition, so the browser can carry elements that
 * exist before and after (the cover into Now Playing) and slide whole pages past each other. Without view
 * transitions, or when the system asks for less motion or the window is hidden, `update` just runs.
 * `before` runs while the old state is still on screen (to name the elements that should travel), `during` once the new
 * state is drawn, `after` when the transition is over. `dir` is which way pages slide (1: forward, -1: back).
 * Returns whether a transition was started.
 */
export function withViewTransition(
  update: () => void,
  opts: { dir?: 1 | -1; before?: () => void; during?: () => void; after?: () => void } = {},
): boolean {
  const start = (document as Document & { startViewTransition?: (cb: () => Promise<void>) => ViewTransitionLike }).startViewTransition;
  if (typeof start !== 'function' || prefersReducedMotion() || document.visibilityState === 'hidden') {
    update();
    return false;
  }
  opts.before?.();
  document.documentElement.style.setProperty('--axis-dir', String(opts.dir ?? 1));
  const quiet = () => {};
  const t = start.call(document, async () => {
    update();
    await tick();
    opts.during?.();
  });
  void t.ready.catch(quiet);
  void t.updateCallbackDone.catch(quiet);
  void t.finished.catch(quiet).finally(() => opts.after?.());
  return true;
}
