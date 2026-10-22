import { beforeEach, describe, expect, it, vi } from 'vitest';
import { groupByDay } from './format';
import { LibraryStore } from './library.svelte';
import { MockBackend } from './mock';
import { snackbar } from './snackbar.svelte';
import type { TrackDto } from './bindings/TrackDto';

const t = (n: string): TrackDto => ({ key: `yt:${n}`, title: `Title ${n}`, artist: 'A', durationMs: 1000, thumbnail: null });
const lastMessage = () => snackbar.items.at(-1)?.message;

beforeEach(() => {
  for (const s of [...snackbar.items]) snackbar.dismiss(s.id);
});

describe('LibraryStore', () => {
  it('mirrors the backend library and updates when it changes', async () => {
    const b = new MockBackend();
    const lib = new LibraryStore();
    await lib.init(b);
    expect(lib.ready).toBe(true);
    expect(lib.isLiked('yt:a')).toBe(false);
    await b.setLiked(t('a'), true);
    expect(lib.isLiked('yt:a')).toBe(true);
    const id = await b.playlistCreate('Mix', [t('a')]);
    expect(lib.playlists.map((p) => p.id)).toEqual([id]);
    expect(lib.revision).toBeGreaterThan(1);
  });

  it('asks for the library again when the page wakes up, not while it is hidden', async () => {
    const b = new MockBackend();
    let reads = 0;
    const read = b.getLibrary.bind(b);
    b.getLibrary = async () => (reads++, read());
    const lib = new LibraryStore();
    await lib.init(b);
    expect(reads).toBe(1);
    const visibility = (hidden: boolean) => {
      Object.defineProperty(document, 'hidden', { configurable: true, get: () => hidden });
      document.dispatchEvent(new Event('visibilitychange'));
    };
    visibility(true);
    await Promise.resolve();
    expect(reads).toBe(1);
    visibility(false);
    await vi.waitFor(() => expect(reads).toBe(2));
  });

  it('toggleLike likes, then unlikes', async () => {
    const lib = new LibraryStore();
    await lib.init(new MockBackend());
    await lib.toggleLike(t('a'));
    expect(lib.isLiked('yt:a')).toBe(true);
    await lib.toggleLike(t('a'));
    expect(lib.isLiked('yt:a')).toBe(false);
  });

  it('says what happened when adding to a playlist', async () => {
    const b = new MockBackend();
    const lib = new LibraryStore();
    await lib.init(b);
    const id = await b.playlistCreate('Road trip', [t('a')]);
    const pl = { id, name: 'Road trip' };
    await lib.addToPlaylist(pl, [t('b')]);
    expect(lastMessage()).toBe('Added to Road trip');
    await lib.addToPlaylist(pl, [t('a')]);
    expect(lastMessage()).toBe('Already in Road trip');
    await lib.addToPlaylist(pl, [t('a'), t('c')]);
    expect(lastMessage()).toBe('Added 1 to Road trip (1 already there)');
  });

  it('shows backend failures instead of swallowing them', async () => {
    const b = new MockBackend();
    const lib = new LibraryStore();
    await lib.init(b);
    await lib.addToPlaylist({ id: 404, name: 'Gone' }, [t('a')]);
    expect(lastMessage()).toBe('That playlist no longer exists.');
  });

  it('carries a library-open error from the backend', async () => {
    const b = new MockBackend();
    b.getLibrary = async () => ({ revision: 1, likedKeys: [], playlists: [], historyCount: 0, error: 'disk is read-only' });
    const lib = new LibraryStore();
    await lib.init(b);
    expect(lib.snapshot.error).toBe('disk is read-only');
  });
});

describe('groupByDay', () => {
  const at = (y: number, m: number, d: number, h = 12) => new Date(y, m - 1, d, h).getTime();
  const now = at(2026, 10, 10, 15);

  it('labels today and yesterday and keeps newest-first order', () => {
    const items = [
      { id: 1, at: at(2026, 10, 10, 14) },
      { id: 2, at: at(2026, 10, 10, 9) },
      { id: 3, at: at(2026, 10, 9, 23) },
    ];
    const g = groupByDay(items, (i) => i.at, now);
    expect(g.map((x) => x.label)).toEqual(['Today', 'Yesterday']);
    expect(g[0]!.items.map((i) => i.id)).toEqual([1, 2]);
  });

  it('uses the local calendar day, so just after midnight counts as yesterday', () => {
    const late = new Date(2026, 9, 9, 23, 59).getTime();
    const early = new Date(2026, 9, 10, 0, 1).getTime();
    const g = groupByDay([{ a: early }, { a: late }], (i) => i.a, new Date(2026, 9, 10, 0, 30).getTime());
    expect(g.map((x) => x.label)).toEqual(['Today', 'Yesterday']);
  });

  it('dates older days and adds the year for old entries', () => {
    const g = groupByDay([{ a: at(2026, 10, 3) }, { a: at(2025, 1, 5) }], (i) => i.a, now);
    expect(g).toHaveLength(2);
    expect(g[0]!.label).not.toMatch(/Today|Yesterday/);
    expect(g[1]!.label).toContain('2025');
  });

  it('returns nothing for no items', () => {
    expect(groupByDay([], () => 0, now)).toEqual([]);
  });
});
