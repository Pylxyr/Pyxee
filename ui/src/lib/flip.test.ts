import { afterEach, describe, expect, it, vi } from 'vitest';
import { flipList, flipMoves } from './flip';

describe('flipMoves', () => {
  it('reports only rows that were there before and moved', () => {
    const before = new Map([['a', 0], ['b', 50], ['c', 100]]);
    const after = new Map([['b', 0], ['c', 100], ['a', 50], ['new', 150]]);
    expect(flipMoves(before, after)).toEqual([
      { key: 'b', dy: 50 },
      { key: 'a', dy: -50 },
    ]);
  });
  it('is empty when nothing moved, or when the list was replaced as a whole', () => {
    expect(flipMoves(new Map([['a', 0]]), new Map([['a', 0.4]]))).toEqual([]);
    expect(flipMoves(new Map([['a', 0], ['b', 50]]), new Map([['x', 0], ['y', 50]]))).toEqual([]);
  });
});

const original = window.matchMedia;
afterEach(() => {
  window.matchMedia = original;
  vi.restoreAllMocks();
  document.body.innerHTML = '';
});

/** A list whose rows' offsets the test sets by hand (jsdom has no layout). */
function setup(tops: number[]) {
  const ul = document.createElement('ul');
  const rows = tops.map((t, i) => {
    const li = document.createElement('li');
    li.id = `r${i}`;
    Object.defineProperty(li, 'offsetTop', { get: () => (li as unknown as { _top: number })._top, configurable: true });
    (li as unknown as { _top: number })._top = t;
    return li;
  });
  Object.defineProperty(ul, 'offsetTop', { value: 0 });
  ul.append(...rows);
  document.body.append(ul);
  const place = (order: HTMLElement[]) => {
    order.forEach((li, i) => ((li as unknown as { _top: number })._top = i * 50));
    ul.append(...order);
  };
  return { ul, rows, place };
}
const tick = () => new Promise((r) => setTimeout(r, 0));

describe('flipList', () => {
  it('glides a moved row with a transform-only animation, and leaves the others alone', async () => {
    const { ul, rows, place } = setup([0, 50, 100]);
    const spy = vi.spyOn(Element.prototype, 'animate');
    const a = flipList(ul);
    place([rows[1]!, rows[0]!, rows[2]!]);
    await tick();
    expect(spy).toHaveBeenCalledTimes(2);
    for (const call of spy.mock.calls) {
      const keyframes = call[0] as Keyframe[];
      expect(keyframes.flatMap((k) => Object.keys(k))).toEqual(['transform', 'transform']);
    }
    expect(spy.mock.calls[0]![0]).toEqual([{ transform: 'translateY(50px)' }, { transform: 'translateY(0)' }]);
    a.destroy();
  });

  it('does not animate a row that arrives, or a list that does not change', async () => {
    const { ul, rows } = setup([0, 50]);
    const spy = vi.spyOn(Element.prototype, 'animate');
    const a = flipList(ul);
    const fresh = document.createElement('li');
    Object.defineProperty(fresh, 'offsetTop', { value: 100 });
    ul.append(fresh);
    await tick();
    expect(spy).not.toHaveBeenCalled();
    expect(rows).toHaveLength(2);
    a.destroy();
  });

  it('stays still under reduced motion and in a hidden window, but keeps measuring', async () => {
    const { ul, rows, place } = setup([0, 50]);
    const spy = vi.spyOn(Element.prototype, 'animate');
    const a = flipList(ul);
    window.matchMedia = ((q: string) => ({ matches: q.includes('prefers-reduced-motion'), media: q })) as unknown as typeof window.matchMedia;
    place([rows[1]!, rows[0]!]);
    await tick();
    expect(spy).not.toHaveBeenCalled();
    window.matchMedia = original;
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
    place([rows[0]!, rows[1]!]);
    await tick();
    expect(spy).not.toHaveBeenCalled();
    hidden.mockReturnValue(false);
    place([rows[1]!, rows[0]!]);
    await tick();
    // measured from the last (hidden) look, so the move is the real one: the first row is 50 px above where it was
    expect(spy.mock.calls[0]![0]).toEqual([{ transform: 'translateY(50px)' }, { transform: 'translateY(0)' }]);
    a.destroy();
  });

  it('stops watching when the list goes away', async () => {
    const { ul, rows, place } = setup([0, 50]);
    const spy = vi.spyOn(Element.prototype, 'animate');
    flipList(ul).destroy();
    place([rows[1]!, rows[0]!]);
    await tick();
    expect(spy).not.toHaveBeenCalled();
  });
});
