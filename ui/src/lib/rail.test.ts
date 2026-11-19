import { afterEach, describe, expect, it, vi } from 'vitest';
import { railScale, toastRail } from './rail';
import type { Rail } from './snackbar.svelte';

describe('railScale', () => {
  it('runs from where it started down to nothing, never outside 0..1', () => {
    expect(railScale(1, 0, 4000)).toBe(1);
    expect(railScale(1, 1000, 4000)).toBe(0.75);
    expect(railScale(0.5, 2000, 4000)).toBe(0.25);
    expect(railScale(1, 9000, 4000)).toBe(0);
    expect(railScale(1, -5, 4000)).toBe(1);
    expect(railScale(1, 10, 0)).toBe(0);
  });
});

const original = window.matchMedia;
afterEach(() => {
  window.matchMedia = original;
  vi.restoreAllMocks();
});

const rail = (p: Partial<Rail> = {}): Rail => ({ ms: 4000, seq: 1, fresh: true, held: false, ...p });
const fake = () => ({ cancel: vi.fn(), pause: vi.fn(), play: vi.fn(), currentTime: 0, playState: 'running' }) as unknown as Animation & { cancel: ReturnType<typeof vi.fn>; pause: ReturnType<typeof vi.fn>; play: ReturnType<typeof vi.fn> };

describe('toastRail', () => {
  it('runs a linear scaleX animation (transform only) for the time the snack has', () => {
    const node = document.createElement('span');
    const spy = vi.spyOn(node, 'animate').mockReturnValue(fake());
    toastRail(node, rail());
    expect(spy).toHaveBeenCalledWith([{ transform: 'scaleX(1)' }, { transform: 'scaleX(0)' }], { duration: 4000, easing: 'linear', fill: 'forwards' });
  });

  it('is held while the snack is held and runs on after, from where it stood, over the time left', () => {
    const node = document.createElement('span');
    const a = fake();
    const spy = vi.spyOn(node, 'animate').mockReturnValue(a);
    const action = toastRail(node, rail());
    (a as unknown as { currentTime: number }).currentTime = 1000; // a quarter of 4 s gone
    action.update(rail({ held: true }));
    expect(a.pause).toHaveBeenCalled();
    (a as unknown as { playState: string }).playState = 'paused';
    action.update(rail({ held: false }));
    expect(a.play).toHaveBeenCalled();
    // the store arms the timer again with what is left (3 s, at least 1 s): the rail carries on from 0.75 over that time
    const b = fake();
    spy.mockReturnValue(b);
    action.update(rail({ seq: 2, ms: 3000, fresh: false }));
    expect(a.cancel).toHaveBeenCalled();
    expect(spy).toHaveBeenLastCalledWith([{ transform: 'scaleX(0.75)' }, { transform: 'scaleX(0)' }], { duration: 3000, easing: 'linear', fill: 'forwards' });
  });

  it('starts full again for the same message shown again', () => {
    const node = document.createElement('span');
    const a = fake();
    const spy = vi.spyOn(node, 'animate').mockReturnValue(a);
    const action = toastRail(node, rail());
    (a as unknown as { currentTime: number }).currentTime = 3000;
    action.update(rail({ seq: 2, fresh: true }));
    expect(spy).toHaveBeenLastCalledWith([{ transform: 'scaleX(1)' }, { transform: 'scaleX(0)' }], expect.anything());
  });

  it('does nothing under reduced motion or without animate, and cancels when the snack goes', () => {
    const node = document.createElement('span');
    window.matchMedia = ((q: string) => ({ matches: q.includes('prefers-reduced-motion'), media: q })) as unknown as typeof window.matchMedia;
    const spy = vi.spyOn(node, 'animate');
    toastRail(node, rail()).destroy();
    expect(spy).not.toHaveBeenCalled();
    window.matchMedia = original;
    const bare = document.createElement('span');
    (bare as unknown as { animate?: unknown }).animate = undefined;
    expect(() => toastRail(bare, rail()).destroy()).not.toThrow();
    const n2 = document.createElement('span');
    const a = fake();
    vi.spyOn(n2, 'animate').mockReturnValue(a);
    toastRail(n2, rail()).destroy();
    expect(a.cancel).toHaveBeenCalled();
  });
});
