import { describe, expect, it } from 'vitest';
import { LyricsStore } from './lyrics.svelte';
import type { Backend } from './backend';
import type { LyricsDto } from './bindings/LyricsDto';
import type { TrackDto } from './bindings/TrackDto';

const track = (key: string): TrackDto => ({ key, title: key, artist: 'x', durationMs: 30_000, thumbnail: null });

/** A backend whose lyrics answers are released by hand, in any order. */
function slowBackend() {
  const pending = new Map<string, (l: LyricsDto) => void>();
  const asked: string[] = [];
  const backend = {
    lyricsGet: (t: TrackDto) =>
      new Promise<LyricsDto>((resolve) => {
        asked.push(t.key);
        pending.set(t.key, resolve);
      }),
  } as unknown as Backend;
  return { backend, pending, asked };
}

describe('LyricsStore', () => {
  it('drops the answer for a song the user has already left', async () => {
    const { backend, pending } = slowBackend();
    const store = new LyricsStore();
    store.init(backend);
    const a = store.load(track('a'));
    const b = store.load(track('b'));
    pending.get('b')!({ kind: 'plain', text: 'B words' });
    await b;
    pending.get('a')!({ kind: 'plain', text: 'A words' }); // arrives late
    await a;
    expect(store.state).toEqual({ kind: 'ready', key: 'b', lyrics: { kind: 'plain', text: 'B words' } });
  });

  it('does not ask again for the same song, until told to retry', async () => {
    const { backend, pending, asked } = slowBackend();
    const store = new LyricsStore();
    store.init(backend);
    const first = store.load(track('a'));
    pending.get('a')!({ kind: 'error', message: 'offline' });
    await first;
    await store.load(track('a'));
    expect(asked).toEqual(['a']);
    const again = store.retry(track('a'));
    pending.get('a')!({ kind: 'notFound' });
    await again;
    expect(asked).toEqual(['a', 'a']);
    expect(store.state).toEqual({ kind: 'ready', key: 'a', lyrics: { kind: 'notFound' } });
  });

  it('loading while a request for the same song is in flight does not ask twice', async () => {
    const { backend, pending, asked } = slowBackend();
    const store = new LyricsStore();
    store.init(backend);
    const p = store.load(track('a'));
    void store.load(track('a'));
    pending.get('a')!({ kind: 'instrumental' });
    await p;
    expect(asked).toEqual(['a']);
  });

  it('no song means no lyrics', async () => {
    const store = new LyricsStore();
    store.init(slowBackend().backend);
    await store.load(null);
    expect(store.state).toEqual({ kind: 'idle' });
  });
  it('turns a failed request into the usual error state instead of loading forever', async () => {
    const backend = { lyricsGet: () => Promise.reject(new Error('ipc closed')) } as unknown as Backend;
    const store = new LyricsStore();
    store.init(backend);
    await store.load(track('a'));
    expect(store.state).toMatchObject({ kind: 'ready', key: 'a', lyrics: { kind: 'error' } });
    // A failure for a song the user already left must not touch the song they are on.
    let fail!: (e: Error) => void;
    const slow = {
      lyricsGet: (t: TrackDto) =>
        t.key === 'x'
          ? new Promise<LyricsDto>((_, reject) => (fail = reject))
          : Promise.resolve<LyricsDto>({ kind: 'plain', text: 'Y words' }),
    } as unknown as Backend;
    const other = new LyricsStore();
    other.init(slow);
    const x = other.load(track('x'));
    await other.load(track('y'));
    fail(new Error('late'));
    await x;
    expect(other.state).toEqual({ kind: 'ready', key: 'y', lyrics: { kind: 'plain', text: 'Y words' } });
  });
});
