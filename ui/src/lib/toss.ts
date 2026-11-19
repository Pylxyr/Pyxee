import { prefersReducedMotion, tokenEase } from './motion';

export interface Pt {
  x: number;
  y: number;
}

const DOT = 10;
const FLIGHT_MS = 460;
const BUMP_MS = 300;

/**
 * Points along a short arc from `from` to `to` (a quadratic curve whose control point sits above both ends), so a
 * thrown song rises and falls instead of sliding in a straight line. Sampled evenly; `steps` segments.
 */
export function tossPath(from: Pt, to: Pt, steps = 6): Pt[] {
  const control = { x: (from.x + to.x) / 2, y: Math.min(from.y, to.y) - 90 };
  const out: Pt[] = [];
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const a = (1 - t) * (1 - t);
    const b = 2 * (1 - t) * t;
    const c = t * t;
    out.push({ x: a * from.x + b * control.x + c * to.x, y: a * from.y + b * control.y + c * to.y });
  }
  return out;
}

/** The queue's door on screen right now: the first `[data-toss-dest]` that is drawn and not behind an overlay. */
function destination(): HTMLElement | null {
  for (const el of document.querySelectorAll<HTMLElement>('[data-toss-dest]')) {
    if (el.getClientRects().length > 0 && !el.closest('[inert]')) return el;
  }
  return null;
}

const centre = (r: DOMRect): Pt => ({ x: r.left + r.width / 2, y: r.top + r.height / 2 });

/**
 * "Play next" and "Add to queue" throw the song at the queue: a small dot of the accent colour leaves the row, arcs to the
 * Queue tab (or its place in the bottom bar) and the tab gives a small bump when it lands. The dot is one fixed 10 px
 * element, moved by one Web Animations `transform`/`opacity` animation of 460 ms, and removed when it ends; the tab's bump is
 * one `transform` animation of 300 ms. Nothing is left running or promoted. The notice that follows still says what
 * happened, so nothing is lost without it: under reduced motion, in a hidden window, without `animate`, or when the queue's
 * door is not on screen, nothing is thrown (returns false).
 */
export function tossToQueue(source: Element | null): boolean {
  if (prefersReducedMotion() || (typeof document !== 'undefined' && document.hidden)) return false;
  const dest = destination();
  if (!dest || typeof dest.animate !== 'function') return false;

  const rect = source?.isConnected ? source.getBoundingClientRect() : null;
  const start: Pt = rect && rect.width > 0 ? centre(rect) : { x: window.innerWidth / 2, y: window.innerHeight * 0.6 };
  const end = centre(dest.getBoundingClientRect());

  const dot = document.createElement('span');
  dot.className = 'toss-dot';
  dot.setAttribute('aria-hidden', 'true');
  document.body.append(dot);
  if (typeof dot.animate !== 'function') {
    dot.remove();
    return false;
  }

  const path = tossPath(start, end);
  const last = path.length - 1;
  const frames: Keyframe[] = path.map((p, i) => ({
    transform: `translate(${p.x - DOT / 2}px, ${p.y - DOT / 2}px) scale(${1 - 0.5 * (i / last)})`,
    opacity: i === last ? 0.3 : 1,
    offset: i / last,
  }));
  const timing = (easing: string): KeyframeAnimationOptions => ({ duration: FLIGHT_MS, easing });
  let flight: Animation;
  try {
    flight = dot.animate(frames, timing(tokenEase('--ease', 'ease-in-out')));
  } catch {
    flight = dot.animate(frames, timing('ease-in-out'));
  }
  const land = () => {
    dot.remove();
    if (dest.isConnected && !prefersReducedMotion()) {
      dest.animate([{ transform: 'scale(1)' }, { transform: 'scale(1.14)', offset: 0.4 }, { transform: 'scale(1)' }], { duration: BUMP_MS, easing: 'ease-out' });
    }
  };
  flight.finished.then(land, () => dot.remove());
  return true;
}
