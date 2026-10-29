import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Backend } from './backend';
import type { SearchFilterDto } from './bindings/SearchFilterDto';
import type { SearchResponseDto } from './bindings/SearchResponseDto';
import { DEMO_TRACKS, MockBackend } from './mock';
import { EMPTY_STATE } from './player.svelte';
import { SearchStore, SUGGEST_DELAY_MS } from './search.svelte';
import { noDiagnostics, noLibrary, noShell } from './test-utils';

/** A backend whose searches resolve when the test says so, to exercise out-of-order answers. */
function controllable() {
  const pending: { query: string; filter: SearchFilterDto; resolve: (r: SearchResponseDto) => void; reject: (e: unknown) => void }[] = [];
  const suggestCalls: string[] = [];
  const backend: Backend = {
    ...noLibrary,
    ...noDiagnostics,
    ...noShell,
    getState: async () => EMPTY_STATE,
    dispatch: async () => {},
    onState: async () => () => {},
    onPosition: async () => () => {},
    windowInfo: async () => ({ backdrop: 'none' }),
    setVolume: async () => {},
    devCatalog: async () => [],
    search: (query, filter) => new Promise((resolve, reject) => void pending.push({ query, filter, resolve, reject })),
    suggest: async (p) => (suggestCalls.push(p), [`${p} one`, `${p} two`]),
    searchFilters: async () => [
      { filter: 'all', supported: true }, { filter: 'songs', supported: true }, { filter: 'albums', supported: false },
    ],
  };
  return { backend, pending, suggestCalls };
}

const track = (n: number) => ({ type: 'track' as const, track: { key: `k${n}`, title: `T${n}`, artist: 'A', durationMs: 1000, thumbnail: null } });

describe('SearchStore', () => {
  it('loads results for a query', async () => {
    const m = new MockBackend();
    m.searchLatencyMs = 0;
    const s = new SearchStore();
    s.init(m);
    await s.run('lowtide');
    expect(s.status).toBe('ready');
    expect(s.items.map((i) => (i.type === 'track' ? i.track.title : ''))).toEqual(['Neon Harbor']);
  });

  it('shows loading while waiting, and an empty query resets to idle', async () => {
    const c = controllable();
    const s = new SearchStore();
    s.init(c.backend);
    const p = s.run('x');
    expect(s.status).toBe('loading');
    c.pending[0]!.resolve({ items: [track(1)] });
    await p;
    expect(s.status).toBe('ready');
    await s.run('   ');
    expect(s.status).toBe('idle');
    expect(s.items).toEqual([]);
  });

  it('ignores a slow answer for an older query', async () => {
    const c = controllable();
    const s = new SearchStore();
    s.init(c.backend);
    const first = s.run('first');
    const second = s.run('second');
    c.pending[1]!.resolve({ items: [track(2)] });
    await second;
    c.pending[0]!.resolve({ items: [track(1)] }); // arrives late
    await first;
    expect(s.query).toBe('second');
    expect(s.items).toEqual([track(2)]);
  });

  it('turns failures into a typed error and can retry', async () => {
    const c = controllable();
    const s = new SearchStore();
    s.init(c.backend);
    const p = s.run('x');
    c.pending[0]!.reject({ kind: 'offline', message: 'offline!' });
    await p;
    expect(s.status).toBe('error');
    expect(s.error).toEqual({ kind: 'offline', message: 'offline!' });
    const r = s.retry();
    expect(s.status).toBe('loading');
    expect(s.error).toBeNull();
    c.pending[1]!.resolve({ items: [] });
    await r;
    expect(s.status).toBe('ready');
    const q = s.run('y');
    c.pending[2]!.reject(new Error('weird'));
    await q;
    expect(s.error).toEqual({ kind: 'other', message: 'weird' });
  });

  it('re-runs the query when a supported filter is chosen and ignores unsupported ones', async () => {
    const c = controllable();
    const s = new SearchStore();
    s.init(c.backend);
    await Promise.resolve();
    await Promise.resolve();
    const p = s.run('x');
    c.pending[0]!.resolve({ items: [] });
    await p;
    const f = s.setFilter('songs');
    expect(c.pending[1]).toMatchObject({ query: 'x', filter: 'songs' });
    c.pending[1]!.resolve({ items: [] });
    await f;
    await s.setFilter('albums');
    expect(c.pending).toHaveLength(2);
    expect(s.filter).toBe('songs');
  });
});

describe('suggestions', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('waits for a pause in typing and asks only once', async () => {
    const c = controllable();
    const s = new SearchStore();
    s.init(c.backend);
    s.typing('lo');
    s.typing('low');
    s.typing('lowt');
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS + 1);
    expect(c.suggestCalls).toEqual(['lowt']);
    expect(s.suggestions).toEqual(['lowt one', 'lowt two']);
  });

  it('clears suggestions for blank input, and running a search hides them', async () => {
    const c = controllable();
    const s = new SearchStore();
    s.init(c.backend);
    s.typing('abc');
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS + 1);
    expect(s.suggestions).toHaveLength(2);
    s.typing('  ');
    expect(s.suggestions).toEqual([]);
    s.typing('abc');
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS + 1);
    void s.run('abc');
    expect(s.suggestions).toEqual([]);
  });

  it('drops suggestions that arrive after the user kept typing or searched', async () => {
    let release!: (v: string[]) => void;
    const slow: Backend = { ...controllable().backend, suggest: () => new Promise((r) => (release = r)) };
    const s = new SearchStore();
    s.init(slow);
    s.typing('abc');
    await vi.advanceTimersByTimeAsync(SUGGEST_DELAY_MS + 1);
    s.clearSuggestions();
    release(['late']);
    await vi.advanceTimersByTimeAsync(0);
    expect(s.suggestions).toEqual([]);
  });
});

describe('mock backend search', () => {
  it('simulates the failures the UI must handle', async () => {
    const m = new MockBackend();
    m.searchLatencyMs = 0;
    await expect(m.search('!offline', 'all')).rejects.toMatchObject({ kind: 'offline' });
    await expect(m.search('!ratelimit', 'all')).rejects.toMatchObject({ kind: 'rateLimited' });
    await expect(m.search('!missing', 'all')).rejects.toMatchObject({ kind: 'toolMissing' });
    await expect(m.search('x', 'albums')).rejects.toMatchObject({ kind: 'unsupported' });
    expect((await m.search('zzzz-no-match', 'all')).items).toEqual([]);
    expect((await m.search('', 'artists')).items.length).toBe(new Set(DEMO_TRACKS.map((t) => t.artist)).size);
  });
});

describe('the remembered search filter', () => {
  const memory = () => {
    const m = new Map<string, string>();
    return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v), m };
  };
  const backendWith = (supported: Record<string, boolean>) =>
    ({
      searchFilters: () => Promise.resolve(Object.entries(supported).map(([filter, ok]) => ({ filter, supported: ok }))),
      search: () => Promise.resolve({ items: [] }),
    }) as never;

  it('starts on Songs', () => {
    expect(new SearchStore().filter).toBe('songs');
    expect(new SearchStore(memory()).filter).toBe('songs');
  });

  it('remembers the filter picked on the chips for the next launch', async () => {
    const mem = memory();
    const a = new SearchStore(mem);
    a.init(backendWith({ all: true, songs: true, videos: true }));
    await Promise.resolve();
    await a.setFilter('videos');
    expect(new SearchStore(mem).filter).toBe('videos');
  });

  it('does not remember a filter that was only used programmatically (Go to artist)', async () => {
    const mem = memory();
    const a = new SearchStore(mem);
    await a.run('', 'albums');
    expect(mem.m.size).toBe(0);
  });

  it('ignores junk in storage and survives a storage that throws', () => {
    const mem = memory();
    mem.m.set('pyxee.searchFilter', 'nonsense');
    expect(new SearchStore(mem).filter).toBe('songs');
    const broken = { getItem: () => { throw new Error('no'); }, setItem: () => { throw new Error('no'); } };
    expect(new SearchStore(broken).filter).toBe('songs');
  });

  it('falls back to Songs when the remembered filter is not supported any more', async () => {
    const mem = memory();
    mem.m.set('pyxee.searchFilter', 'albums');
    const s = new SearchStore(mem);
    s.init(backendWith({ all: true, songs: true, albums: false }));
    await new Promise((r) => setTimeout(r, 0));
    expect(s.filter).toBe('songs');
  });
});
