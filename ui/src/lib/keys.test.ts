import { describe, expect, it } from 'vitest';
import { withKeys } from './keys';

describe('withKeys', () => {
  it('uses the song key, and counts a song that appears twice', () => {
    const rows = withKeys(['a', 'b', 'a', 'a'], (x) => x);
    expect(rows.map((r) => r.key)).toEqual(['a', 'b', 'a#1', 'a#2']);
    expect(rows.map((r) => r.index)).toEqual([0, 1, 2, 3]);
  });
  it('a row keeps its key when rows before it come and go', () => {
    const before = withKeys(['x', 'a', 'b'], (s) => s).find((r) => r.item === 'b')!.key;
    const after = withKeys(['a', 'b'], (s) => s).find((r) => r.item === 'b')!.key;
    expect(after).toBe(before);
  });
});
