import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import LyricsPanel from './LyricsPanel.svelte';
import { EMPTY_STATE, player } from '$lib/player.svelte';
import { lyricsStore } from '$lib/lyrics.svelte';

const track = { key: 'k1', title: 'T', artist: 'A', durationMs: 60_000, thumbnail: null };
const lines = [
  { timeMs: 1_000, text: 'one' },
  { timeMs: 2_500, text: 'two' },
  { timeMs: 4_000, text: 'three' },
];

function show(transport: 'playing' | 'paused', positionMs: number) {
  player.state = {
    ...EMPTY_STATE,
    seq: 1,
    transport: transport === 'playing' ? { state: 'playing', id: 1 } : { state: 'paused', id: 1, loaded: true },
    audible: 1,
    current: 1,
    positionMs,
    queue: [{ id: 1, origin: 'user', track }],
  };
  lyricsStore.state = { kind: 'ready', key: 'k1', lyrics: { kind: 'synced', lines } };
}
const current = (c: HTMLElement) => c.querySelector('[data-current="true"]')?.textContent?.trim();

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  player.state = EMPTY_STATE;
  lyricsStore.state = { kind: 'idle' };
});

describe('the lyric line being sung', () => {
  it('changes at its own time between the once-a-second position reports', async () => {
    show('playing', 1_000); // the last report: 1.0 s, line "one"
    const { container } = render(LyricsPanel, { active: true });
    flushSync();
    expect(current(container)).toBe('one');
    // No report arrives. The next line starts at 2.5 s: 1.5 s from the report.
    await vi.advanceTimersByTimeAsync(1_400);
    flushSync();
    expect(current(container)).toBe('one');
    await vi.advanceTimersByTimeAsync(150);
    flushSync();
    expect(current(container)).toBe('two');
    // The next report (2.55 s) arrives; the line after it starts at 4 s, 1.45 s later.
    show('playing', 2_550);
    flushSync();
    await vi.advanceTimersByTimeAsync(1_400);
    flushSync();
    expect(current(container)).toBe('two');
    await vi.advanceTimersByTimeAsync(100);
    flushSync();
    expect(current(container)).toBe('three');
  });

  it('does not keep timing off an old clock when the reports have stopped', async () => {
    show('playing', 1_000);
    render(LyricsPanel, { active: true });
    flushSync();
    await vi.advanceTimersByTimeAsync(10_000); // no report for 10 s: the window was asleep
    flushSync();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('sets no timer while paused', () => {
    show('paused', 1_000);
    render(LyricsPanel, { active: true });
    flushSync();
    expect(vi.getTimerCount()).toBe(0);
  });
});
