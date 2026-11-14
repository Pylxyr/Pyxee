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
});
