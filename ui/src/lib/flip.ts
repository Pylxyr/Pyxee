import { MOTION, prefersReducedMotion } from './motion';

/** Rows that moved fewer pixels than this are left alone (rounding noise, not a move). */
const MIN_MOVE_PX = 1;
/** At most this many rows glide at once: the ones on screen are all a person can see move. */
const MAX_ANIMATED = 40;
const FALLBACK_EASE = 'cubic-bezier(0.16, 1, 0.3, 1)';

/**
 * Which rows moved, and by how much: the pixels each row was from where it is now (the FLIP "invert"). Only rows that
 * were in the list before AND are now count: a row that arrives or leaves has its own enter or exit, and a list
 * replaced as a whole has no row in common, so nothing glides.
 */
export function flipMoves<K>(before: Map<K, number>, after: Map<K, number>): { key: K; dy: number }[] {
  const out: { key: K; dy: number }[] = [];
  for (const [key, top] of after) {
    const was = before.get(key);
    if (was === undefined) continue;
    const dy = was - top;
    if (Math.abs(dy) >= MIN_MOVE_PX) out.push({ key, dy });
  }
  return out;
}

const ease = () =>
  (typeof getComputedStyle === 'function' ? getComputedStyle(document.documentElement).getPropertyValue('--ease-out').trim() : '') || FALLBACK_EASE;

/**
 * Svelte action for a list of rows (the queue, Up next, the lane, a playlist): when rows are reordered, or the ones
 * below a removed row close the gap, they glide to their new place instead of snapping. A queue then feels like
 * something with weight: Play next slides the song up, and when the next song starts the rest of Up next moves up one
 * row, like a conveyor.
 *
 * How: the rows' offsets are kept from the last look; when the list's children change (a MutationObserver, so a row
 * that Svelte removes after its exit animation counts too) each row that stayed is animated from its old offset with
 * one Web Animations `transform` animation (compositor only, ends by itself, nothing promoted before or after it).
 * Nothing runs while the list is idle. Under reduced motion, in a hidden window, or without `animate` the offsets are
 * still kept and nothing moves.
 */
export function flipList(node: HTMLElement) {
  const rows = () => [...node.children].filter((c): c is HTMLElement => c instanceof HTMLElement);
  const measure = () => {
    const base = node.offsetTop;
    return new Map(rows().map((el) => [el, el.offsetTop - base] as const));
  };
  let before = measure();
  const running = new Set<Animation>();

  const play = () => {
    const after = measure();
    const moves = flipMoves(before, after);
    before = after;
    if (moves.length === 0 || prefersReducedMotion() || (typeof document !== 'undefined' && document.hidden)) return;
    const easing = ease();
    let n = 0;
    for (const { key: el, dy } of moves) {
      if (n >= MAX_ANIMATED || typeof el.animate !== 'function') break;
      const r = el.getBoundingClientRect();
      if (r.bottom < -r.height || r.top > window.innerHeight + r.height) continue; // far off screen: nobody sees it move
      let anim: Animation;
      try {
        anim = el.animate([{ transform: `translateY(${dy}px)` }, { transform: 'translateY(0)' }], { duration: MOTION.slow, easing });
      } catch {
        anim = el.animate([{ transform: `translateY(${dy}px)` }, { transform: 'translateY(0)' }], { duration: MOTION.slow, easing: 'ease-out' });
      }
      running.add(anim);
      const done = () => running.delete(anim);
      anim.finished?.then(done, done);
      n++;
    }
  };

  // A row that is already gliding is restarted from where the layout now puts it (a quick second move is a small jump, not a glitch).
  const mo = new MutationObserver(() => {
    for (const a of running) a.cancel();
    running.clear();
    play();
  });
  mo.observe(node, { childList: true });
  // A resize changes offsets without changing the children: keep the baseline honest so the next move is measured from it.
  const ro = typeof ResizeObserver === 'function' ? new ResizeObserver(() => (before = measure())) : null;
  ro?.observe(node);

  return {
    destroy() {
      mo.disconnect();
      ro?.disconnect();
      for (const a of running) a.cancel();
      running.clear();
    },
  };
}
