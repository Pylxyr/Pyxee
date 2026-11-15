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

/**
 * A short pop on an element (transform only, a Web Animations animation that ends by itself, so nothing is promoted
 * before or after it). Nothing under reduced motion or where there is no `animate`.
 */
export function pulse(el: Element, scale = 1.28): void {
  if (prefersReducedMotion() || typeof el.animate !== 'function') return;
  el.animate([{ transform: 'scale(0.78)' }, { transform: `scale(${scale})`, offset: 0.45 }, { transform: 'scale(1)' }], {
    duration: 380,
    easing: 'cubic-bezier(0.2, 0.8, 0.2, 1)',
  });
}

/**
 * Svelte action: pops the element when `value` changes (not when it first appears). Used for the heart: liking a song
 * answers with a small pop, while a list that is drawn with hearts already set stays still.
 */
export function popOnChange(node: HTMLElement, value: unknown) {
  let last = value;
  return {
    update(next: unknown) {
      if (next !== last) {
        last = next;
        pulse(node);
      }
    },
  };
}

// A row that leaves or comes back animates only when the person just did that (removed it, pressed Undo). A list that is
// replaced as a whole (a new queue, new search results) swaps without it: old rows fading out while the new ones stand
// below them would show two lists for a moment.
let listChangedAt = -Infinity;
const LIST_CHANGE_WINDOW_MS = 700;

/** Call when the person removes a row or undoes a removal, just before the list changes. */
export function markListChange(): void {
  listChangedAt = typeof performance !== 'undefined' ? performance.now() : Date.now();
}

/** `ms` for a row's enter or exit, or 0 when the list was not just changed by the person (and under reduced motion). */
export function listChangeMs(ms: number): number {
  const now = typeof performance !== 'undefined' ? performance.now() : Date.now();
  return now - listChangedAt < LIST_CHANGE_WINDOW_MS ? motionMs(ms) : 0;
}

/**
 * Runs `update` (the state change that brings another song's cover) as a view transition in which the old cover and the
 * new one are the same named element, `cover-now`, so the browser dissolves one into the other on the compositor
 * (a pair of snapshots fading, nothing re-laid-out). The cover is the one on the deck or the dock; with Now Playing open
 * it is not on screen and `update` just runs, as it does without view transitions.
 */
export function crossfadeCover(update: () => void): boolean {
  const cover = () => {
    const el = document.querySelector<HTMLElement>('[data-np-source]');
    return el && el.getClientRects().length > 0 && !el.closest('[inert]') ? el : null;
  };
  if (!cover()) {
    update();
    return false;
  }
  let named: HTMLElement | null = null;
  const name = () => {
    named?.style.removeProperty('view-transition-name');
    named = cover();
    named?.style.setProperty('view-transition-name', 'cover-now');
  };
  return withViewTransition(update, { before: name, during: name, after: () => named?.style.removeProperty('view-transition-name') });
}
