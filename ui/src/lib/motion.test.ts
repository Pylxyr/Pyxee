import { afterEach, describe, expect, it, vi } from 'vitest';
import { nav } from './nav.svelte';
import { MOTION, crossfadeCover, motionMs, motionPx, prefersReducedMotion, springOut, springPop, swapOnChange, tokenEase, turn, turnOnChange, withViewTransition } from './motion';

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

describe('toggle answers (shuffle swap, repeat turn)', () => {
  const setup = () => {
    const wrap = document.createElement('span');
    wrap.innerHTML = '<button><svg></svg></button>';
    document.body.append(wrap);
    return { wrap, svg: wrap.querySelector('svg')! };
  };
  afterEach(() => {
    vi.restoreAllMocks();
    document.body.innerHTML = '';
  });

  it('animate the icon, with transform only, when the value changes and not when it first appears or stays', async () => {
    system(false);
    const { wrap, svg } = setup();
    const spy = vi.spyOn(svg, 'animate');
    const action = swapOnChange(wrap, false);
    action.update(false);
    expect(spy).not.toHaveBeenCalled();
    action.update(true);
    expect(spy).toHaveBeenCalledTimes(1);
    const keys = (spy.mock.calls[0]![0] as Keyframe[]).flatMap((k) => Object.keys(k).filter((p) => p !== 'offset'));
    expect([...new Set(keys)]).toEqual(['transform']);
  });

  it('turns the repeat icon forwards, and backwards when repeat goes off', () => {
    system(false);
    const { wrap, svg } = setup();
    const spy = vi.spyOn(svg, 'animate');
    const action = turnOnChange(wrap, 'off');
    action.update('all');
    action.update('one');
    action.update('off');
    expect(spy.mock.calls.map((c) => (c[0] as Keyframe[])[1]!.transform)).toEqual(['rotate(360deg)', 'rotate(360deg)', 'rotate(-360deg)']);
  });

  it('do nothing under reduced motion, in a hidden window, or where there is no animate', () => {
    const { wrap, svg } = setup();
    const spy = vi.spyOn(svg, 'animate');
    const action = turnOnChange(wrap, 'off');
    system(true);
    action.update('all');
    system(false);
    vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
    action.update('one');
    expect(spy).not.toHaveBeenCalled();
    const bare = document.createElement('span');
    (bare as unknown as { animate?: unknown }).animate = undefined;
    expect(() => turn(bare)).not.toThrow();
  });

  it('fall back to a plain easing when the page cannot read the token, and when the webview rejects it', () => {
    system(false);
    const { svg } = setup();
    expect(tokenEase('--nope', 'ease-out')).toBe('ease-out');
    const spy = vi.spyOn(svg, 'animate').mockImplementationOnce(() => {
      throw new TypeError('bad easing');
    });
    turn(svg);
    expect(spy).toHaveBeenCalledTimes(2);
    expect((spy.mock.calls[1]![1] as KeyframeAnimationOptions).easing).toBe('cubic-bezier(0.2, 0.8, 0.2, 1)');
  });
});
