import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Backend } from './backend';
import { RELATED_BLOCK_MS, RELATED_DELAY_MS, RelatedStore } from './related.svelte';
import { DEMO_TRACKS, MockBackend } from './mock';

const [A, B, C] = [DEMO_TRACKS[0]!, DEMO_TRACKS[1]!, DEMO_TRACKS[2]!];

function setup() {
  const m = new MockBackend();
  const s = new RelatedStore();
  s.init(m as Backend);
  return { m, s };
}
const settle = () => vi.advanceTimersByTimeAsync(RELATED_DELAY_MS + 1);

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('RelatedStore', () => {
  it('is idle with nothing playing', () => {
    const { s } = setup();
    s.load(null);
    expect(s.state.kind).toBe('idle');
  });

  it('asks once, after a short wait, and never lists the seed', async () => {
    const { m, s } = setup();
    s.load(A);
    expect(s.state.kind).toBe('loading');
    expect(m.relatedCalls).toEqual([]);
    await settle();
    expect(m.relatedCalls).toEqual([A.key]);
    expect(s.state.kind).toBe('ready');
    if (s.state.kind === 'ready') expect(s.state.tracks.map((t) => t.key)).not.toContain(A.key);
  });

  it('skipping through songs asks only for the one that is left on', async () => {
    const { m, s } = setup();
    s.load(A);
    await vi.advanceTimersByTimeAsync(100);
    s.load(B);
    await vi.advanceTimersByTimeAsync(100);
    s.load(C);
    await settle();
    expect(m.relatedCalls).toEqual([C.key]);
  });

  it('asking again for the same song costs nothing, before and after the answer', async () => {
    const { m, s } = setup();
    s.load(A);
    s.load(A);
    await settle();
    s.load(A);
    s.load(B);
    await settle();
    s.load(A);
    expect(m.relatedCalls).toEqual([A.key, B.key]);
    expect(s.state.kind).toBe('ready');
  });

  it('drops the answer for a song the user has left', async () => {
    const { m, s } = setup();
    let release: () => void = () => {};
    const gate = new Promise<void>((r) => (release = r));
    const original = m.relatedGet.bind(m);
    m.relatedGet = async (key, count) => {
      await gate;
      return original(key, count);
    };
    s.load(A);
    await settle();
    s.load(B);
    release();
    await settle();
    expect(s.state).toMatchObject({ kind: 'ready', key: B.key });
  });

  it('an error waits for Try again instead of asking on its own', async () => {
    const { m, s } = setup();
    m.relatedFailure = { kind: 'other', message: 'Boom.' };
    s.load(A);
    await settle();
    expect(s.state).toMatchObject({ kind: 'error', error: { message: 'Boom.' } });
    s.load(A);
    await settle();
    expect(m.relatedCalls).toHaveLength(1);
    m.relatedFailure = null;
    s.retry(A);
    await vi.advanceTimersByTimeAsync(1);
    expect(m.relatedCalls).toHaveLength(2);
    expect(s.state.kind).toBe('ready');
  });

  it('offline or rate limited: other songs show the message without asking, until the pause is over or Try again', async () => {
    const { m, s } = setup();
    m.relatedFailure = { kind: 'rateLimited', message: 'Slow down.' };
    s.load(A);
    await settle();
    s.load(B);
    await settle();
    expect(m.relatedCalls).toEqual([A.key]);
    expect(s.state).toMatchObject({ kind: 'error', key: B.key, error: { message: 'Slow down.' } });
    await vi.advanceTimersByTimeAsync(RELATED_BLOCK_MS);
    m.relatedFailure = null;
    s.load(C);
    await settle();
    expect(m.relatedCalls).toEqual([A.key, C.key]);
    expect(s.state.kind).toBe('ready');
  });

  it('Try again goes through during the pause', async () => {
    const { m, s } = setup();
    m.relatedFailure = { kind: 'offline', message: 'Offline.' };
    s.load(A);
    await settle();
    m.relatedFailure = null;
    s.retry(A);
    await vi.advanceTimersByTimeAsync(1);
    expect(s.state.kind).toBe('ready');
  });

  it('an empty answer is a result, not an error', async () => {
    const { m, s } = setup();
    m.relatedGet = async () => [];
    s.load(A);
    await settle();
    expect(s.state).toMatchObject({ kind: 'ready', tracks: [] });
  });
});
