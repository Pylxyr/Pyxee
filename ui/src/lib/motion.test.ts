import { afterEach, describe, expect, it, vi } from 'vitest';
import { nav } from './nav.svelte';
import { MOTION, crossfadeCover, motionMs, motionPx, prefersReducedMotion, springOut, springPop, withViewTransition } from './motion';

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

  describe('with view transitions', () => {
    const doc = document as unknown as { startViewTransition?: unknown };
    const fake = () =>
      vi.fn((cb: () => Promise<void>) => {
        const done = Promise.resolve().then(cb);
        return { ready: done, updateCallbackDone: done, finished: done };
      });
    function visible() {
      Object.defineProperty(document, 'visibilityState', { value: 'visible', configurable: true });
    }
    afterEach(() => {
      delete doc.startViewTransition;
      delete (document as unknown as { visibilityState?: string }).visibilityState;
      nav.route = 'home';
    });

    it('runs the update inside it, between before and after', async () => {
      system(false);
      visible();
      doc.startViewTransition = fake();
      const order: string[] = [];
      const started = withViewTransition(() => order.push('update'), {
        before: () => order.push('before'),
        during: () => order.push('during'),
        after: () => order.push('after'),
      });
      expect(started).toBe(true);
      await vi.waitFor(() => expect(order).toEqual(['before', 'update', 'during', 'after']));
    });

    describe('the cover dissolve', () => {
      const cover = (html = '<span data-np-source>cover</span>') => {
        document.body.innerHTML = html;
        const el = document.querySelector<HTMLElement>('[data-np-source]')!;
        el.getClientRects = () => [{}] as unknown as DOMRectList; // jsdom has no layout: say it is on screen
        return el;
      };

      it('names the old and the new cover the same, runs the update between, and removes the name after', async () => {
        system(false);
        visible();
        doc.startViewTransition = fake();
        const old = cover();
        let fresh: HTMLElement | null = null;
        const started = crossfadeCover(() => {
          expect(old.style.getPropertyValue('view-transition-name')).toBe('cover-now');
          old.remove();
          fresh = cover('<span data-np-source>new</span>');
        });
        expect(started).toBe(true);
        await vi.waitFor(() => expect(fresh).not.toBeNull());
        await vi.waitFor(() => expect(fresh!.style.getPropertyValue('view-transition-name')).toBe(''));
      });

      it('just runs the update when there is no cover on screen, or with Now Playing over it', () => {
        system(false);
        visible();
        const start = fake();
        doc.startViewTransition = start;
        document.body.innerHTML = '';
        let ran = 0;
        expect(crossfadeCover(() => ran++)).toBe(false);
        cover('<div inert><span data-np-source>c</span></div>');
        expect(crossfadeCover(() => ran++)).toBe(false);
        expect(ran).toBe(2);
        expect(start).not.toHaveBeenCalled();
      });

      it('just runs the update where view transitions do not exist', () => {
        system(false);
        cover();
        let ran = 0;
        expect(crossfadeCover(() => ran++)).toBe(false);
        expect(ran).toBe(1);
      });
    });

    it('a search while on the search page, or a playlist while on a playlist, slides nothing', () => {
      system(false);
      visible();
      const start = fake();
      doc.startViewTransition = start;
      nav.route = 'search';
      nav.search('lowtide');
      expect(nav.query).toBe('lowtide');
      expect(start).not.toHaveBeenCalled();
      nav.route = 'home';
      nav.search('lowtide');
      expect(start).toHaveBeenCalledTimes(1);
    });
  });
});
