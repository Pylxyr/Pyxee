import { describe, expect, it } from 'vitest';
import { DEFAULT_LOOK, lookAttributes, parseLook } from './look';

describe('parseLook', () => {
  it('keeps what is known', () => {
    expect(parseLook({ corners: 'square', hero: 'vinyl' })).toEqual({ ...DEFAULT_LOOK, corners: 'square', hero: 'vinyl' });
  });
  it('falls back to the default for anything unknown or damaged', () => {
    expect(parseLook({ corners: 'hexagon', hero: 3, surfaces: null })).toEqual(DEFAULT_LOOK);
    expect(parseLook(null)).toEqual(DEFAULT_LOOK);
    expect(parseLook('flat')).toEqual(DEFAULT_LOOK);
  });
});

describe('lookAttributes', () => {
  it('leaves the defaults off the root element', () => {
    expect(Object.values(lookAttributes(DEFAULT_LOOK)).every((v) => v === null)).toBe(true);
  });
  it('names each other choice as data-<option>', () => {
    const a = lookAttributes({ ...DEFAULT_LOOK, surfaces: 'flat', covers: 'circle' });
    expect(a['data-surfaces']).toBe('flat');
    expect(a['data-covers']).toBe('circle');
    expect(a['data-corners']).toBeNull();
  });
});
