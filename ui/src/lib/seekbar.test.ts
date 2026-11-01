import { describe, expect, it } from 'vitest';
import { devicePixels, nextPixelDelayMs, parseSeekMode, pixelOf, pixelPct, stepsFor } from './seekbar';

describe('seek bar pixels', () => {
  // 200 s song on a 420 px bar: one pixel every 476.19 ms
  const dur = 200_000;
  const w = 420;

  it('knows the whole pixel a position reaches', () => {
    expect(pixelOf(0, dur, w)).toBe(0);
    expect(pixelOf(476, dur, w)).toBe(0);
    expect(pixelOf(477, dur, w)).toBe(1);
    expect(pixelOf(dur, dur, w)).toBe(w);
    expect(pixelOf(dur * 2, dur, w)).toBe(w);
    expect(pixelOf(-5, dur, w)).toBe(0);
  });

  it('snaps the percentage down to a pixel, so it only changes when a pixel does', () => {
    expect(pixelPct(477, dur, w)).toBeCloseTo(100 / 420, 6);
    expect(pixelPct(477, dur, w)).toBe(pixelPct(900, dur, w));
    expect(pixelPct(100, 0, w)).toBe(0);
    expect(pixelPct(100, dur, 0)).toBe(0);
  });

  it('waits exactly until the next pixel boundary', () => {
    expect(nextPixelDelayMs(0, dur, w)).toBe(477); // boundary at 476.19, rounded up so the pixel has changed
    const d = nextPixelDelayMs(1000, dur, w)!;
    expect(pixelOf(1000 + d, dur, w)).toBe(pixelOf(1000, dur, w) + 1);
    expect(pixelOf(1000 + d - 1, dur, w)).toBe(pixelOf(1000, dur, w));
  });

  it('every step it schedules is one pixel, over a whole song', () => {
    let pos = 0;
    let timers = 0;
    let last = 0;
    while (true) {
      const d = nextPixelDelayMs(pos, dur, w);
      if (d == null) break;
      pos += d;
      timers++;
      const px = pixelOf(pos, dur, w);
      expect(px - last).toBe(1);
      last = px;
    }
    expect(timers).toBe(w);
  });

  it('has nothing to wait for with no length, no width, or a full bar', () => {
    expect(nextPixelDelayMs(0, 0, w)).toBeNull();
    expect(nextPixelDelayMs(0, dur, 0)).toBeNull();
    expect(nextPixelDelayMs(dur, dur, w)).toBeNull();
  });

  it('turns a CSS width into device pixels', () => {
    expect(devicePixels(420, 1)).toBe(420);
    expect(devicePixels(420.4, 1.5)).toBe(631);
    expect(devicePixels(-3, 1)).toBe(0);
  });

  it('divides the rest of the song into a few steps a second, never finer than a pixel', () => {
    expect(stepsFor(10_000, 420)).toBe(40);
    expect(stepsFor(10_000, 12)).toBe(12);
    expect(stepsFor(0, 420)).toBe(1);
  });

  it('reads the setting, and takes anything unknown as the plain step', () => {
    expect(parseSeekMode(' Pixel ')).toBe('pixel');
    expect(parseSeekMode('glide')).toBe('glide');
    expect(parseSeekMode('steps')).toBe('steps');
    expect(parseSeekMode('')).toBe('step');
    expect(parseSeekMode('wobble')).toBe('step');
    expect(parseSeekMode(null)).toBe('step');
  });
});
