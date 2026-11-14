import { describe, expect, it } from 'vitest';
import { contrast, dominantColor, ensureContrast } from './color';

function pixels(...px: [number, number, number, number][]): Uint8ClampedArray {
  return new Uint8ClampedArray(px.flat());
}
const repeat = (p: [number, number, number, number], n: number) => Array.from({ length: n }, () => p);

describe('dominantColor', () => {
  it('prefers the vivid colour over a larger grey area', () => {
    const data = pixels(...repeat([120, 120, 120, 255], 60), ...repeat([200, 40, 60, 255], 40));
    const hex = dominantColor(data)!;
    const r = parseInt(hex.slice(1, 3), 16), g = parseInt(hex.slice(3, 5), 16);
    expect(r).toBeGreaterThan(g + 80); // clearly red-ish
  });
  it('ignores transparent pixels, and near-black and near-white ones when there is anything else', () => {
    const data = pixels(...repeat([0, 0, 0, 255], 20), ...repeat([255, 255, 255, 255], 20), ...repeat([200, 40, 60, 255], 5), ...repeat([200, 40, 60, 0], 20));
    expect(dominantColor(data)).toBe('#c8283c');
  });
  it('still finds a colour in a cover that is only dark or only light', () => {
    expect(dominantColor(pixels(...repeat([20, 22, 30, 255], 50)))).toBe('#14161e');
    expect(dominantColor(pixels(...repeat([240, 240, 235, 255], 50)))).toBe('#f0f0eb');
  });
  it('has no colour for a picture that is pure black, pure white or transparent', () => {
    expect(dominantColor(pixels(...repeat([0, 0, 0, 255], 20), ...repeat([255, 255, 255, 255], 20), ...repeat([200, 40, 60, 0], 20)))).toBeNull();
  });
  it('still finds a colour in a dark or a pale cover instead of giving up', () => {
    const dark = dominantColor(pixels(...repeat([24, 26, 40, 255], 50), ...repeat([0, 0, 0, 255], 50)));
    expect(dark).not.toBeNull();
    expect(parseInt(dark!.slice(5, 7), 16)).toBeGreaterThan(parseInt(dark!.slice(1, 3), 16)); // bluish
    const pale = dominantColor(pixels(...repeat([244, 238, 238, 255], 50), ...repeat([255, 255, 255, 255], 50)));
    expect(pale).not.toBeNull();
  });
  it('prefers a real colour to the fallback when the cover has one', () => {
    const data = pixels(...repeat([20, 20, 20, 255], 80), ...repeat([200, 40, 60, 255], 5));
    expect(dominantColor(data)).toBe('#c8283c');
  });
  it('returns null for empty input', () => {
    expect(dominantColor(new Uint8ClampedArray())).toBeNull();
  });
});

describe('ensureContrast', () => {
  it('lightens a dark colour until it reads on the dark theme', () => {
    const fixed = ensureContrast('#1a1a40', 'dark');
    expect(contrast(fixed, '#0f0f0f')).toBeGreaterThanOrEqual(3);
  });
  it('darkens a pale colour for the light theme', () => {
    const fixed = ensureContrast('#ffe9a0', 'light');
    expect(contrast(fixed, '#ffffff')).toBeGreaterThanOrEqual(3);
  });
  it('leaves an already readable colour alone', () => {
    expect(ensureContrast('#ff3b4a', 'dark')).toBe('#ff3b4a');
  });
});
