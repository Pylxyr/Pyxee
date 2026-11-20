import { describe, expect, it } from 'vitest';
import { isUiScale, zoomFor } from './uiscale';

describe('the interface size', () => {
  it('a fixed choice is that percentage', () => {
    expect(zoomFor('100', 3840, false)).toBe(1);
    expect(zoomFor('125', 1280, true)).toBe(1.25);
    expect(zoomFor('200', 1280, false)).toBe(2);
  });

  it('automatic leaves ordinary and already scaled screens alone', () => {
    expect(zoomFor('auto', 1920, false)).toBe(1);
    expect(zoomFor('auto', 2399, false)).toBe(1);
    expect(zoomFor('auto', 3840, true)).toBe(1);
  });

  it('automatic enlarges an unscaled very wide window, in quarter steps, up to 200%', () => {
    expect(zoomFor('auto', 2560, false)).toBe(1.25);
    expect(zoomFor('auto', 3840, false)).toBe(2);
    expect(zoomFor('auto', 7680, false)).toBe(2);
  });

  it('knows its own values', () => {
    expect(isUiScale('auto')).toBe(true);
    expect(isUiScale('150')).toBe(true);
    expect(isUiScale('300')).toBe(false);
    expect(isUiScale(null)).toBe(false);
  });
});
