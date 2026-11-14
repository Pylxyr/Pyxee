import { describe, expect, it } from 'vitest';
import { currentLine, sleepLabel } from './lyrics';

const lines = [
  { timeMs: 1000, text: 'a' },
  { timeMs: 5000, text: 'b' },
  { timeMs: 9000, text: 'c' },
];

describe('currentLine', () => {
  it('is the last line that has started, and -1 before the first', () => {
    expect(currentLine(lines, 0)).toBe(-1);
    expect(currentLine(lines, 999)).toBe(-1);
    expect(currentLine(lines, 1000)).toBe(0);
    expect(currentLine(lines, 4999)).toBe(0);
    expect(currentLine(lines, 5000)).toBe(1);
    expect(currentLine(lines, 9_999_999)).toBe(2);
    expect(currentLine([], 5)).toBe(-1);
  });

  it('copes with equal timestamps (the later line wins)', () => {
    const l = [{ timeMs: 1000, text: 'a' }, { timeMs: 1000, text: 'b' }];
    expect(currentLine(l, 1000)).toBe(1);
  });
});

describe('sleepLabel', () => {
  it('describes each timer in plain words', () => {
    expect(sleepLabel(null)).toBe('Off');
    expect(sleepLabel({ kind: 'minutes', remainingMs: 28 * 60_000 })).toBe('28 min left');
    expect(sleepLabel({ kind: 'minutes', remainingMs: 61_000 })).toBe('2 min left');
    expect(sleepLabel({ kind: 'minutes', remainingMs: 45_000 })).toBe('45 s left');
    expect(sleepLabel({ kind: 'minutes', remainingMs: 0 })).toBe('0 s left');
    expect(sleepLabel({ kind: 'endOfTrack' })).toBe('End of song');
    expect(sleepLabel({ kind: 'tracks', left: 3 })).toBe('3 songs left');
    expect(sleepLabel({ kind: 'tracks', left: 1 })).toBe('Last song');
  });
});
