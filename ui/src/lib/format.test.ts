import { describe, expect, it } from 'vitest';
import { formatTime, hueFor } from './format';

describe('formatTime', () => {
  it('formats minutes and seconds', () => {
    expect(formatTime(0)).toBe('0:00');
    expect(formatTime(83_000)).toBe('1:23');
    expect(formatTime(59_999)).toBe('0:59');
  });
  it('adds hours when needed', () => {
    expect(formatTime(3_725_000)).toBe('1:02:05');
  });
  it('shows a placeholder for unknown durations', () => {
    expect(formatTime(null)).toBe('–:––');
    expect(formatTime(undefined)).toBe('–:––');
    expect(formatTime(-5)).toBe('–:––');
    expect(formatTime(Number.NaN)).toBe('–:––');
  });
});

describe('hueFor', () => {
  it('is stable and in range', () => {
    expect(hueFor('demo:1')).toBe(hueFor('demo:1'));
    for (const k of ['a', 'demo:2', 'yt:abcdef']) {
      const h = hueFor(k);
      expect(h).toBeGreaterThanOrEqual(0);
      expect(h).toBeLessThan(360);
    }
  });
});
