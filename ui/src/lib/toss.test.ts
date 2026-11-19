import { afterEach, describe, expect, it, vi } from 'vitest';
import { tossPath, tossToQueue } from './toss';

describe('tossPath', () => {
  it('starts and ends where it is told and rises above both ends in between', () => {
    const p = tossPath({ x: 10, y: 300 }, { x: 410, y: 40 });
    expect(p).toHaveLength(7);
    expect(p[0]).toEqual({ x: 10, y: 300 });
    expect(p[6]).toEqual({ x: 410, y: 40 });
    expect(Math.min(...p.map((q) => q.y))).toBeLessThan(40);
    expect(p.map((q) => q.x)).toEqual([...p.map((q) => q.x)].sort((a, b) => a - b));
  });
});

const original = window.matchMedia;
afterEach(() => {
  window.matchMedia = original;
  vi.restoreAllMocks();
  document.body.innerHTML = '';
});

function page(extra = '') {
  document.body.innerHTML = `<div><button id="row">row</button></div><div ${extra}><button id="queue" data-toss-dest>Queue</button></div>`;
  const row = document.getElementById('row')!;
  const dest = document.getElementById('queue')!;
  row.getBoundingClientRect = () => ({ left: 0, top: 200, width: 100, height: 40 }) as DOMRect;
  dest.getBoundingClientRect = () => ({ left: 300, top: 0, width: 80, height: 32 }) as DOMRect;
  dest.getClientRects = () => [{}] as unknown as DOMRectList;
  return { row, dest };
}
const tick = () => new Promise((r) => setTimeout(r, 0));

describe('tossToQueue', () => {
  it('flies one dot with transform and opacity only, removes it, and bumps the queue tab when it lands', async () => {
    const { row, dest } = page();
    const flight = vi.spyOn(Element.prototype, 'animate');
    expect(tossToQueue(row)).toBe(true);
    expect(document.querySelectorAll('.toss-dot')).toHaveLength(1);
    const frames = flight.mock.calls[0]![0] as Keyframe[];
    expect([...new Set(frames.flatMap((f) => Object.keys(f).filter((k) => k !== 'offset')))].sort()).toEqual(['opacity', 'transform']);
    await tick();
    expect(document.querySelector('.toss-dot')).toBeNull();
    const bump = flight.mock.calls[1]!;
    expect(flight.mock.contexts[1]).toBe(dest);
    expect((bump[0] as Keyframe[]).every((f) => Object.keys(f).every((k) => k === 'transform' || k === 'offset'))).toBe(true);
  });

  it('throws nothing under reduced motion, in a hidden window, without a door on screen, or with it behind an overlay', () => {
    const { row } = page();
    const spy = vi.spyOn(Element.prototype, 'animate');
    window.matchMedia = ((q: string) => ({ matches: q.includes('prefers-reduced-motion'), media: q })) as unknown as typeof window.matchMedia;
    expect(tossToQueue(row)).toBe(false);
    window.matchMedia = original;
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
    expect(tossToQueue(row)).toBe(false);
    hidden.mockReturnValue(false);
    page('inert');
    expect(tossToQueue(row)).toBe(false);
    document.body.innerHTML = '<button id="row">r</button>';
    expect(tossToQueue(document.getElementById('row'))).toBe(false);
    expect(spy).not.toHaveBeenCalled();
    expect(document.querySelector('.toss-dot')).toBeNull();
  });

  it('starts from the middle of the window when the row it came from is gone', () => {
    const { dest } = page();
    const spy = vi.spyOn(Element.prototype, 'animate');
    expect(tossToQueue(null)).toBe(true);
    const first = (spy.mock.calls[0]![0] as Keyframe[])[0]!.transform as string;
    expect(first).toContain(`${window.innerWidth / 2 - 5}px`);
    expect(dest.isConnected).toBe(true);
  });
});
