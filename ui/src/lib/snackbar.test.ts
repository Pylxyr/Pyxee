import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SnackbarStore } from './snackbar.svelte';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('SnackbarStore', () => {
  it('shows a message and auto-dismisses it', () => {
    const s = new SnackbarStore();
    s.push('Added to queue');
    expect(s.items.map((i) => i.message)).toEqual(['Added to queue']);
    vi.advanceTimersByTime(4001);
    expect(s.items).toHaveLength(0);
  });
  it('keeps messages with an Undo action up longer', () => {
    const s = new SnackbarStore();
    s.push('Removed', { action: { label: 'Undo', run: () => {} } });
    vi.advanceTimersByTime(4001);
    expect(s.items).toHaveLength(1);
    vi.advanceTimersByTime(3000);
    expect(s.items).toHaveLength(0);
  });
  it('runs the action once and closes the snack', () => {
    const s = new SnackbarStore();
    const run = vi.fn();
    const id = s.push('Removed', { action: { label: 'Undo', run } });
    s.act(id);
    s.act(id);
    expect(run).toHaveBeenCalledTimes(1);
    expect(s.items).toHaveLength(0);
  });
  it('shows at most three at a time, newest last', () => {
    const s = new SnackbarStore();
    for (const m of ['a', 'b', 'c', 'd']) s.push(m);
    expect(s.items.map((i) => i.message)).toEqual(['b', 'c', 'd']);
  });
  it('the same plain message is not stacked twice: its timer starts again', () => {
    const s = new SnackbarStore();
    const a = s.push('Could not reach the network');
    vi.advanceTimersByTime(3000);
    const b = s.push('Could not reach the network');
    expect(b).toBe(a);
    expect(s.items).toHaveLength(1);
    vi.advanceTimersByTime(3000);
    expect(s.items).toHaveLength(1);
    vi.advanceTimersByTime(1100);
    expect(s.items).toHaveLength(0);
  });
  it('two messages with an Undo each stay separate', () => {
    const s = new SnackbarStore();
    s.push('Removed', { action: { label: 'Undo', run: () => {} } });
    s.push('Removed', { action: { label: 'Undo', run: () => {} } });
    expect(s.items).toHaveLength(2);
  });
  it('a snack waits while the pointer or focus is on it, then runs out its time', () => {
    const s = new SnackbarStore();
    const id = s.push('Added to queue');
    vi.advanceTimersByTime(2000);
    s.pause(id);
    vi.advanceTimersByTime(60_000);
    expect(s.items).toHaveLength(1);
    s.resume(id);
    vi.advanceTimersByTime(1900);
    expect(s.items).toHaveLength(1);
    vi.advanceTimersByTime(200);
    expect(s.items).toHaveLength(0);
  });

  describe('the time rail', () => {
    it('follows the timer: full and fresh for a new snack, the same length as its timeout', () => {
      const s = new SnackbarStore();
      const id = s.push('Removed', { action: { label: 'Undo', run: () => {} } });
      expect(s.rails[id]).toEqual({ ms: 7000, seq: 1, fresh: true, held: false });
      s.dismiss(id);
      expect(s.rails[id]).toBeUndefined();
    });
    it('is held with the snack and carries on (not fresh) over the time left, at least a second', () => {
      const s = new SnackbarStore();
      const id = s.push('Hello');
      vi.advanceTimersByTime(1000);
      s.pause(id);
      expect(s.rails[id]).toMatchObject({ held: true, seq: 1 });
      s.resume(id);
      expect(s.rails[id]).toEqual({ ms: 3000, seq: 2, fresh: false, held: false });
      vi.advanceTimersByTime(3001);
      expect(s.items).toHaveLength(0);
      expect(s.rails[id]).toBeUndefined();
    });
    it('starts full again when the same message is shown again', () => {
      const s = new SnackbarStore();
      const id = s.push('Could not load');
      s.push('Could not load');
      expect(s.rails[id]).toMatchObject({ seq: 2, fresh: true });
    });
  });
});
