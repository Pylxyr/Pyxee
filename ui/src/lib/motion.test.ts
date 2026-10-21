import { afterEach, describe, expect, it } from 'vitest';
import { MOTION, motionMs, motionPx, prefersReducedMotion } from './motion';

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
});
