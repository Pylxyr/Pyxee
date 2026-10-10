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
  it('ignores transparent, near-black and near-white pixels', () => {
    const data = pixels(...repeat([0, 0, 0, 255], 20), ...repeat([255, 255, 255, 255], 20), ...repeat([200, 40, 60, 0], 20));
    expect(dominantColor(data)).toBeNull();
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
