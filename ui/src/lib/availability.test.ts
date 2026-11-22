import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AvailabilityStore } from './availability.svelte';
import { library } from './library.svelte';
import { MockBackend } from './mock';

describe('AvailabilityStore', () => {
  let backend: MockBackend;
  let asked: string[][];

  beforeEach(async () => {
    vi.useFakeTimers();
    backend = new MockBackend();
    asked = [];
    // The mock reports keys that contain "missing" as gone.
    const ask = backend.localMissing.bind(backend);
    backend.localMissing = async (keys) => (asked.push(keys), ask(keys));
    await library.init(backend);
  });
  afterEach(() => vi.useRealTimers());

  it('asks once for everything the rows want, and marks only the songs whose file is gone', async () => {
    const store = new AvailabilityStore();
    for (const k of ['file:C:\\Music\\a.mp3', 'file:C:\\Music\\missing.mp3', 'file:C:\\Music\\b.mp3']) store.want(k);
    expect(asked).toEqual([]); // gathered first
    await vi.advanceTimersByTimeAsync(100);
    expect(asked).toHaveLength(1);
    expect(asked[0]).toHaveLength(3);
    expect(store.isMissing('file:C:\\Music\\missing.mp3')).toBe(true);
    expect(store.isMissing('file:C:\\Music\\a.mp3')).toBe(false);
  });

  it('never asks about a YouTube song, and not again for a song looked at a moment ago', async () => {
    const store = new AvailabilityStore();
    store.want('yt:abcdefghijk');
    store.want('file:C:\\Music\\a.mp3');
    await vi.advanceTimersByTimeAsync(100);
    expect(asked).toEqual([['file:C:\\Music\\a.mp3']]);
    store.want('file:C:\\Music\\a.mp3');
    await vi.advanceTimersByTimeAsync(100);
    expect(asked).toHaveLength(1);
    // After a scan the rows look again, and a song that came back is no longer marked.
    store.reset();
    store.want('file:C:\\Music\\a.mp3');
    await vi.advanceTimersByTimeAsync(100);
    expect(asked).toHaveLength(2);
  });

  it('marks nothing when the check fails', async () => {
    backend.localMissing = async () => {
      throw new Error('no');
    };
    const store = new AvailabilityStore();
    store.want('file:C:\\Music\\missing.mp3');
    await vi.advanceTimersByTimeAsync(100);
    expect(store.isMissing('file:C:\\Music\\missing.mp3')).toBe(false);
  });
});
