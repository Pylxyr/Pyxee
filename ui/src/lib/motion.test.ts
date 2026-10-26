import { afterEach, describe, expect, it } from 'vitest';
import { MOTION, motionMs, motionPx, prefersReducedMotion, springOut, springPop, withViewTransition } from './motion';

const original = window.matchMedia;
function system(reduce: boolean) {
  window.matchMedia = ((q: string) => ({ matches: reduce && q.includes('prefers-reduced-motion'), media: q })) as unknown as typeof window.matchMedia;
}
afterEach(() => (window.matchMedia = original));

describe('motion', () => {
  it('keeps the given lengths when the system does not ask for less motion', () => {
    system(false);
    expect(prefersReducedMotion()).toBe(false);
    expect(motionMs(MOTION.base)).toBe(200);
    expect(motionPx(40)).toBe(40);
  });

  it('drops lengths and distances to nothing when it does', () => {
    system(true);
    expect(prefersReducedMotion()).toBe(true);
    expect(motionMs(MOTION.base)).toBe(0);
    expect(motionPx(40)).toBe(0);
  });

  it('leaving is quicker than arriving', () => {
    expect(MOTION.exit).toBeLessThan(MOTION.base);
    expect(MOTION.fast).toBeLessThan(MOTION.base);
  });

  it('the springs start at 0, end at 1, and overshoot a little on the way', () => {
    for (const spring of [springOut, springPop]) {
      expect(spring(0)).toBe(0);
      expect(spring(1)).toBe(1);
      const peak = Math.max(...Array.from({ length: 100 }, (_, i) => spring(i / 99)));
      expect(peak).toBeGreaterThan(1.02);
      expect(peak).toBeLessThan(1.2);
    }
  });

  it('runs the update at once when there are no view transitions (or less motion is asked for)', () => {
    system(false);
    let ran = 0;
    expect(withViewTransition(() => ran++)).toBe(false);
    expect(ran).toBe(1);
  });
});
