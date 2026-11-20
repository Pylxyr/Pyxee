import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';

type Motion = typeof import('./motion');
type Mounted = { destroy(): void } | undefined;

/** jsdom has no IntersectionObserver: a stand-in that reports only what a test tells it to. */
class FakeObserver {
  static all: FakeObserver[] = [];
  targets = new Set<Element>();
  disconnected = false;
  constructor(
    public callback: IntersectionObserverCallback,
    public options?: IntersectionObserverInit,
  ) {
    FakeObserver.all.push(this);
  }
  observe(el: Element) {
    this.targets.add(el);
  }
  unobserve(el: Element) {
    this.targets.delete(el);
  }
  disconnect() {
    this.targets.clear();
    this.disconnected = true;
  }
  fire(entries: unknown[]) {
    this.callback(entries as IntersectionObserverEntry[], this as unknown as IntersectionObserver);
  }
}

const rect = (top: number, left = 0, height = 56) => ({ top, left, height, bottom: top + height, width: 300 });
/** An observer entry for `target`. `cutAtTop`: the window hides the top of it (it is scrolling in from above). */
function entry(target: Element, o: { top?: number; left?: number; intersecting?: boolean; cutAtTop?: boolean } = {}) {
  const top = o.top ?? 100;
  const left = o.left ?? 0;
  return {
    target,
    isIntersecting: o.intersecting ?? true,
    boundingClientRect: rect(top, left),
    intersectionRect: rect(o.cutAtTop ? top + 24 : top, left),
  };
}

const original = window.matchMedia;
function system(reduce: boolean) {
  window.matchMedia = ((q: string) => ({ matches: reduce && q.includes('prefers-reduced-motion'), media: q })) as unknown as typeof window.matchMedia;
}

let motion: Motion;
let animate: MockInstance<typeof Element.prototype.animate>;
const mounted: Mounted[] = [];

/** `n` list items, each with a cover inside, with the action on them. */
function rows(n = 1): HTMLElement[] {
  return Array.from({ length: n }, () => {
    const li = document.createElement('li');
    li.innerHTML = '<span data-cover></span><span class="trow-title">Song</span>';
    document.body.append(li);
    mounted.push(motion.reveal(li));
    return li;
  });
}
const observer = () => FakeObserver.all[0]!;
/** The animations started on `el`: [keyframes, options] each. */
const playedOn = (el: Element) =>
  animate.mock.calls.flatMap((c, i) => (animate.mock.contexts[i] === el ? [[c[0], c[1]] as [Keyframe[], KeyframeAnimationOptions]] : []));
const props = (keyframes: Keyframe[]) => [...new Set(keyframes.flatMap((k) => Object.keys(k)))].sort();

beforeEach(async () => {
  vi.resetModules(); // the list-change clock and the shared observer are module state: each test starts without them
  motion = await import('./motion');
  FakeObserver.all = [];
  vi.stubGlobal('IntersectionObserver', FakeObserver);
  animate = vi.spyOn(Element.prototype, 'animate');
});
afterEach(() => {
  for (const m of mounted.splice(0)) m?.destroy();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  window.matchMedia = original;
  document.body.innerHTML = '';
});

describe('a row reveals itself as it scrolls into view', () => {
  it('waits invisible while it is out of view, and plays once when it comes in', () => {
    const [li] = rows();
    expect(li!.hasAttribute('data-reveal')).toBe(true);
    expect(observer().targets.has(li!)).toBe(true);

    observer().fire([entry(li!, { intersecting: false })]);
    expect(li!.hasAttribute('data-reveal')).toBe(true);
    expect(animate).not.toHaveBeenCalled();

    observer().fire([entry(li!)]);
    expect(li!.hasAttribute('data-reveal')).toBe(false);
    const [row] = playedOn(li!);
    expect(playedOn(li!)).toHaveLength(1);
    expect(props(row![0])).toEqual(['opacity', 'transform']);
    // the first frame is held during the delay and nothing is held after the end: no fill kept, no repeat
    expect(row![1].fill).toBe('backwards');
    expect(row![1].iterations).toBeUndefined();
  });

  it('counts a row as arrived a little inside the window edge, so it is seen playing', () => {
    rows();
    expect(observer().options?.rootMargin).toMatch(/^0px 0px -\d+px 0px$/);
  });

  it('pops the cover in just after its row, with transform and opacity only', () => {
    const [li] = rows();
    observer().fire([entry(li!)]);
    const row = playedOn(li!)[0]!;
    const cover = playedOn(li!.querySelector('[data-cover]')!);
    expect(cover).toHaveLength(1);
    expect(props(cover[0]![0])).toEqual(['opacity', 'transform']);
    expect(cover[0]![1].fill).toBe('backwards');
    expect(cover[0]![1].delay!).toBeGreaterThan(row[1].delay!);
    // nothing else inside the row is animated
    expect(playedOn(li!.querySelector('.trow-title')!)).toHaveLength(0);
  });

  it('starts rows that arrive together one after another, in reading order', () => {
    const [a, b, c] = rows(3);
    // c is lowest; a and b share a line, a to the left of b. They are reported in no particular order.
    observer().fire([entry(c!, { top: 300 }), entry(b!, { top: 100, left: 200 }), entry(a!, { top: 100, left: 0 })]);
    const delay = (el: Element) => playedOn(el)[0]![1].delay!;
    expect(delay(a!)).toBe(0);
    expect(delay(b!)).toBeGreaterThan(delay(a!));
    expect(delay(c!)).toBeGreaterThan(delay(b!));
  });

  it('keeps a whole page of rows within one short wave, however many come in at once', () => {
    const list = rows(40);
    observer().fire(list.map((li, i) => entry(li, { top: 20 + i * 56 })));
    const delays = list.map((li) => playedOn(li)[0]![1].delay!);
    expect(delays[0]).toBe(0);
    expect(Math.max(...delays)).toBeLessThanOrEqual(280);
    expect(delays).toEqual([...delays].sort((x, y) => x - y));
  });

  it('rises from below, or drops in from above when it is scrolling in from the top', () => {
    const [below, above] = rows(2);
    observer().fire([entry(below!), entry(above!, { cutAtTop: true })]);
    const start = (el: Element) => String(playedOn(el)[0]![0][0]!.transform);
    expect(start(below!)).toMatch(/^translateY\(\d/);
    expect(start(above!)).toMatch(/^translateY\(-\d/);
  });

  it('lets go of a row once it has played: nothing observed, nothing replayed, the observer gone with the last one', () => {
    const [a, b] = rows(2);
    observer().fire([entry(a!)]);
    expect(observer().targets.has(a!)).toBe(false);
    expect(observer().targets.has(b!)).toBe(true);
    expect(observer().disconnected).toBe(false);

    const played = animate.mock.calls.length;
    observer().fire([entry(a!)]); // reported again (it scrolled out and back): nothing happens
    expect(animate.mock.calls.length).toBe(played);

    observer().fire([entry(b!)]);
    expect(observer().disconnected).toBe(true);

    // a row that comes later gets a fresh observer
    const [late] = rows();
    expect(FakeObserver.all).toHaveLength(2);
    expect(FakeObserver.all[1]!.targets.has(late!)).toBe(true);
  });

  it('forgets a row that is removed before it was seen', () => {
    const [a, b] = rows(2);
    mounted[0]!.destroy();
    expect(observer().targets.has(a!)).toBe(false);
    expect(observer().targets.has(b!)).toBe(true);
    mounted[1]!.destroy();
    expect(observer().disconnected).toBe(true);
  });
});

describe('a row is never left invisible, and never animates when it should not', () => {
  it('is simply there under reduced motion', () => {
    system(true);
    const [li] = rows();
    expect(li!.hasAttribute('data-reveal')).toBe(false);
    expect(FakeObserver.all).toHaveLength(0);
  });

  it('is simply there where there is no IntersectionObserver, or no animate', () => {
    vi.stubGlobal('IntersectionObserver', undefined);
    const [a] = rows();
    expect(a!.hasAttribute('data-reveal')).toBe(false);
    vi.stubGlobal('IntersectionObserver', FakeObserver);
    const bare = document.createElement('li');
    (bare as unknown as { animate?: unknown }).animate = undefined;
    mounted.push(motion.reveal(bare));
    expect(bare.hasAttribute('data-reveal')).toBe(false);
    expect(FakeObserver.all).toHaveLength(0);
  });

  it('leaves a row that comes back after Undo to its own enter', () => {
    motion.markListChange();
    const [li] = rows();
    expect(li!.hasAttribute('data-reveal')).toBe(false);
  });

  it('appears without animation when the system asked for less motion after the row was drawn', () => {
    const [li] = rows();
    system(true);
    observer().fire([entry(li!)]);
    expect(li!.hasAttribute('data-reveal')).toBe(false);
    expect(animate).not.toHaveBeenCalled();
  });

  it('appears without animation in a hidden window', () => {
    const [li] = rows();
    vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
    observer().fire([entry(li!)]);
    expect(li!.hasAttribute('data-reveal')).toBe(false);
    expect(animate).not.toHaveBeenCalled();
  });

  it('retries with a plain easing when the webview rejects the spring', () => {
    const [li] = rows();
    animate.mockImplementationOnce(() => {
      throw new TypeError('bad easing');
    });
    observer().fire([entry(li!)]);
    expect(playedOn(li!)).toHaveLength(2);
    expect(li!.hasAttribute('data-reveal')).toBe(false);
  });

  it('shows every row of a batch even if the webview refuses to animate at all', () => {
    const [a, b] = rows(2);
    animate.mockImplementation(() => {
      throw new TypeError('no animations here');
    });
    expect(() => observer().fire([entry(a!, { top: 100 }), entry(b!, { top: 200 })])).not.toThrow();
    expect(a!.hasAttribute('data-reveal')).toBe(false);
    expect(b!.hasAttribute('data-reveal')).toBe(false);
  });
});
