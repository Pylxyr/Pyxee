import { afterEach, describe, expect, it, vi } from 'vitest';
import { BACKDROP_PX, backdropCandidates, paintBackdrop } from './backdrop';

/** A 2D context that only records what is drawn (jsdom has no canvas). */
function fakeContext() {
  const calls: { op: string; args: unknown[] }[] = [];
  const ctx = new Proxy(
    {},
    {
      get: (_t, name: string) => (...args: unknown[]) => calls.push({ op: name, args }),
      set: () => true,
    },
  );
  return { ctx, calls };
}

afterEach(() => vi.restoreAllMocks());

describe('backdrop', () => {
  it('asks for a small picture, never the window-sized still', () => {
    expect(backdropCandidates(null)).toEqual([]);
    expect(backdropCandidates('https://i.ytimg.com/vi/abc/maxresdefault.jpg')).toEqual(['https://i.ytimg.com/vi/abc/mqdefault.jpg']);
    expect(backdropCandidates('data:image/svg+xml,x')).toEqual(['data:image/svg+xml,x']);
  });

  it('shrinks the cover to a few pixels and grows it back in doublings onto the canvas', () => {
    const sizes: number[] = [];
    const draws: number[] = [];
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function (this: HTMLCanvasElement) {
      const { ctx } = fakeContext();
      const self = this;
      return new Proxy(ctx, {
        get: (_t, name: string) =>
          name === 'drawImage'
            ? (...a: unknown[]) => {
                sizes.push(self.width);
                draws.push(a.length === 9 ? (a[7] as number) : (a[3] as number));
              }
            : () => {},
        set: () => true,
      }) as unknown as CanvasRenderingContext2D;
    });
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = BACKDROP_PX;
    const cover = { naturalWidth: 320, naturalHeight: 180 } as unknown as HTMLImageElement;
    paintBackdrop(canvas, cover, 10);
    // 12 -> 24 -> 48 on scratch canvases, then the final 96 on the canvas itself: nothing is ever drawn large.
    expect(draws).toEqual([12, 24, 48, BACKDROP_PX]);
    expect(Math.max(...draws)).toBeLessThanOrEqual(BACKDROP_PX);
  });

  it('paints the flat placeholder colour without a picture, and does nothing without a 2D context', () => {
    const { ctx, calls } = fakeContext();
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = BACKDROP_PX;
    vi.spyOn(canvas, 'getContext').mockReturnValue(ctx as unknown as CanvasRenderingContext2D);
    paintBackdrop(canvas, null, 200);
    expect(calls.map((c) => c.op)).toEqual(['clearRect', 'fillRect']);
    const none = document.createElement('canvas');
    vi.spyOn(none, 'getContext').mockReturnValue(null);
    expect(() => paintBackdrop(none, null, 1)).not.toThrow();
  });
});
