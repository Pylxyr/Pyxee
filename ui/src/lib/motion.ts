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
 * A theme the person has just chosen takes over by a crossfade of the whole window (the root view transition: two
 * snapshots faded into each other on the compositor), not by a cut. While it runs the accent's own 600 ms glide is held
 * off (`data-theme-fade` on <html>, see app.css) so the two do not blur into each other. Only for a choice the person
 * makes: the palette that follows the cover each song, and the live preview in the theme editor, never come through here
 * (a new view transition cancels the one running, so they must not queue behind a song's cover dissolve). Without view
 * transitions, or under reduced motion, the update just runs.
 */
export function crossfadeTheme(update: () => void): boolean {
  const root = document.documentElement;
  return withViewTransition(update, {
    before: () => root.setAttribute('data-theme-fade', ''),
    after: () => root.removeAttribute('data-theme-fade'),
  });
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
        if (next === true) burst(node);
      }
    },
  };
}

/**
 * A ring in the accent colour that grows out of the element and fades, once (a "like" answering with a small burst).
 * One short Web Animation on its own layer, gone when it ends; nothing stays running, and nothing at all with reduced motion.
 */
export function burst(node: HTMLElement): void {
  if (prefersReducedMotion() || typeof node.animate !== 'function' || typeof document === 'undefined') return;
  const ring = document.createElement('span');
  ring.setAttribute('aria-hidden', 'true');
  ring.style.cssText =
    'position:absolute;inset:0;border-radius:9999px;pointer-events:none;border:2px solid var(--accent);opacity:0;';
  const restore = getComputedStyle(node).position === 'static' ? node.style.position : null;
  if (restore !== null) node.style.position = 'relative';
  node.appendChild(ring);
  const done = () => {
    ring.remove();
    if (restore !== null) node.style.position = restore;
  };
  ring
    .animate([{ transform: 'scale(0.6)', opacity: 0.9 }, { transform: 'scale(1.9)', opacity: 0 }], {
      duration: 520,
      easing: 'cubic-bezier(0.2, 0.8, 0.2, 1)',
    })
    .addEventListener('finish', done, { once: true });
  // If the animation is cancelled (the row is removed), the ring goes with its parent; this only covers a kept element.
  setTimeout(done, 700);
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

/** A CSS easing token (`--spring`, `--ease-out`...) for script; `fallback` where the page cannot give it (a test, an old webview). */
export function tokenEase(name: string, fallback: string): string {
  if (typeof getComputedStyle !== 'function') return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

/**
 * One-shot transform animation on an element (Web Animations: it ends by itself, so nothing is promoted before or after).
 * Nothing under reduced motion, in a hidden window, or where there is no `animate`. A webview that cannot read the
 * token's easing gets the fallback instead of an error.
 */
function oneShot(el: Element, keyframes: Keyframe[], duration: number, token: string, fallback: string): void {
  if (prefersReducedMotion() || typeof el.animate !== 'function' || (typeof document !== 'undefined' && document.hidden)) return;
  try {
    el.animate(keyframes, { duration, easing: tokenEase(token, fallback) });
  } catch {
    el.animate(keyframes, { duration, easing: fallback });
  }
}

/** Repeat: the icon turns once (backwards when repeat goes off), on the spring. */
export function turn(el: Element, dir: 1 | -1 = 1): void {
  oneShot(el, [{ transform: 'rotate(0deg)' }, { transform: `rotate(${dir * 360}deg)` }], 520, '--spring', 'cubic-bezier(0.2, 0.8, 0.2, 1)');
}

/** Shuffle: the two arrows swap places, a short sideways shuffle and back. */
export function shuffleSwap(el: Element): void {
  oneShot(
    el,
    [{ transform: 'translateX(0)' }, { transform: 'translateX(-4px) scale(0.92)', offset: 0.3 }, { transform: 'translateX(3px)', offset: 0.65 }, { transform: 'translateX(0)' }],
    380,
    '--ease',
    'cubic-bezier(0.2, 0.8, 0.2, 1)',
  );
}

/** The part of a button that moves: its icon, or the element itself when there is none. */
const iconOf = (node: HTMLElement): Element => node.querySelector('svg') ?? node;

function onChange<T>(run: (node: HTMLElement, next: T, prev: T) => void) {
  return (node: HTMLElement, value: T) => {
    let last = value;
    return {
      update(next: T) {
        if (next !== last) {
          const prev = last;
          last = next;
          run(node, next, prev);
        }
      },
    };
  };
}

/** Svelte action: the icon inside answers when `value` changes (not when it first appears). Shuffle uses this. */
export const swapOnChange = onChange<boolean>((node) => shuffleSwap(iconOf(node)));
/** Svelte action for the repeat button: its icon turns when the mode changes (`off` turns it back). */
export const turnOnChange = onChange<string>((node, next) => turn(iconOf(node), next === 'off' ? -1 : 1));

// ---- Reveal on scroll -----------------------------------------------------------------------------------------------
// A row or a cover tile waits invisible (`data-reveal`, `opacity: 0` in app.css) until it scrolls into view, then plays
// once: the row rises on a spring and its cover pops in just after it, spinning a little. However long the list is, only
// what a person scrolls to is animated, and nothing is kept afterwards: the mark is removed as the animation starts, the
// animation has no fill after its end, and the row is dropped from the one shared observer (which disconnects when no
// row is left waiting).

/** A row counts as arrived once its edge is this far inside the window's bottom, so the person sees it play. */
const REVEAL_BOTTOM_PX = 16;
/** Rows that arrive together (a page opening, a quick scroll) start one after another, this far apart at most... */
const REVEAL_STEP_MS = 34;
/** ...and the last of them starts within this, however many arrive at once. */
const REVEAL_WAVE_MS = 280;
const REVEAL_ROW_MS = 460;
const REVEAL_COVER_MS = 520;
/** The cover pops this long after its row starts, so the row is there to catch it. */
const REVEAL_COVER_LAG_MS = 70;
const REVEAL_RISE_PX = 16;
/** Where the page cannot give the springs: a plain curve that also overshoots a little. */
const REVEAL_FALLBACK_EASE = 'cubic-bezier(0.34, 1.56, 0.64, 1)';

let watcher: IntersectionObserver | null = null;
const waiting = new Set<Element>();

function stopWatching(node: Element): void {
  if (!waiting.delete(node)) return;
  watcher?.unobserve(node);
  if (waiting.size === 0) {
    watcher?.disconnect();
    watcher = null;
  }
}

/** One Web Animations animation that ends by itself. `backwards` holds the first frame during `delay` and nothing after the end. */
function playOnce(el: Element, keyframes: Keyframe[], duration: number, delay: number, easing: string): void {
  const options: KeyframeAnimationOptions = { duration, delay, fill: 'backwards' };
  try {
    el.animate(keyframes, { ...options, easing });
  } catch {
    el.animate(keyframes, { ...options, easing: REVEAL_FALLBACK_EASE });
  }
}

/** `from` is the side the row came in from: 1 from below (the usual way), -1 from above (scrolling back up). */
function playReveal(node: HTMLElement, delay: number, from: 1 | -1, spring: string, pop: string): void {
  playOnce(
    node,
    [
      { opacity: 0, transform: `translateY(${from * REVEAL_RISE_PX}px) scale(0.96)` },
      { opacity: 1, transform: 'translateY(0) scale(1)' },
    ],
    REVEAL_ROW_MS,
    delay,
    spring,
  );
  for (const cover of node.querySelectorAll('[data-cover]')) {
    playOnce(
      cover,
      [
        { opacity: 0, transform: `scale(0.4) rotate(${from * -12}deg)` },
        { opacity: 1, transform: 'scale(1) rotate(0deg)' },
      ],
      REVEAL_COVER_MS,
      delay + REVEAL_COVER_LAG_MS,
      pop,
    );
  }
}

function arrive(entries: IntersectionObserverEntry[]): void {
  // Top to bottom, then left to right, so a wave runs through a list (or a row of tiles) in reading order.
  const here = entries
    .filter((e) => e.isIntersecting && waiting.has(e.target))
    .sort((a, b) => Math.round(a.boundingClientRect.top) - Math.round(b.boundingClientRect.top) || a.boundingClientRect.left - b.boundingClientRect.left);
  if (here.length === 0) return;
  // Under reduced motion (switched on after the row was drawn) or in a hidden window the rows just appear.
  const animate = !prefersReducedMotion() && !(typeof document !== 'undefined' && document.hidden);
  const step = Math.min(REVEAL_STEP_MS, REVEAL_WAVE_MS / here.length);
  const spring = animate ? tokenEase('--spring', REVEAL_FALLBACK_EASE) : '';
  const pop = animate ? tokenEase('--spring-pop', REVEAL_FALLBACK_EASE) : '';
  here.forEach((entry, i) => {
    const node = entry.target as HTMLElement;
    stopWatching(node);
    try {
      // A row whose top edge is cut off by the window came in from above; one that is whole (a page opening) or cut at the bottom, from below.
      if (animate) playReveal(node, i * step, entry.intersectionRect.top > entry.boundingClientRect.top + 1 ? -1 : 1, spring, pop);
    } catch {
      // The webview refused the animation: the row just appears (and the rows after it in this batch still get theirs).
    }
    // Whatever happened above, a row is never left invisible.
    delete node.dataset.reveal;
  });
}

/**
 * Svelte action for a list row or a cover tile: it stays invisible until it scrolls into view, then rises in on a spring
 * while its cover (the `[data-cover]` inside, see Thumb) pops in after it. Rows that come into view together start one
 * after another, in reading order. Plays once per row; nothing is left on the row afterwards, and nothing is observed for a
 * row that has played. Where there is nothing to wait for or animate with (reduced motion, no IntersectionObserver or
 * `animate`), the row is simply there. A row that comes back after the person just removed one (Undo) has its own enter,
 * so it is left to that.
 */
export function reveal(node: HTMLElement): { destroy(): void } | undefined {
  if (prefersReducedMotion() || typeof IntersectionObserver !== 'function' || typeof node.animate !== 'function' || listChangeMs(1) > 0) return undefined;
  node.dataset.reveal = '';
  waiting.add(node);
  watcher ??= new IntersectionObserver(arrive, { rootMargin: `0px 0px -${REVEAL_BOTTOM_PX}px 0px` });
  watcher.observe(node);
  return { destroy: () => stopWatching(node) };
}
