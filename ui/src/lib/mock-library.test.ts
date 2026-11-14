import { describe, expect, it } from 'vitest';
import type { TrackDto } from './bindings/TrackDto';
import { MockLibrary } from './mock-library';

const t = (n: string): TrackDto => ({ key: `yt:${n}`, title: `Title ${n}`, artist: 'A', durationMs: 1000, thumbnail: null });
const titles = async (l: MockLibrary, id: number) => (await l.playlistGet(id)).entries.map((e) => e.track.title);

// These mirror the Rust library's tests: the mock must follow the same rules, or UI tests prove nothing.
describe('MockLibrary follows the Rust library rules', () => {
  it('liking is idempotent and newest first; unliking removes only that song', async () => {
    const l = new MockLibrary();
    await l.setLiked(t('a'), true);
    await l.setLiked(t('b'), true);
    await l.setLiked(t('a'), true);
    expect((await l.getLibrary()).likedKeys).toEqual(['yt:b', 'yt:a']);
    await l.setLiked(t('a'), false);
    expect((await l.likedGet()).map((x) => x.title)).toEqual(['Title b']);
  });

  it('playlist names are trimmed and must be 1 to 100 characters', async () => {
    const l = new MockLibrary();
    const id = await l.playlistCreate('  Road trip ', []);
    expect((await l.playlistGet(id)).name).toBe('Road trip');
    await expect(l.playlistCreate('   ', [])).rejects.toBe('Playlist names must be 1 to 100 characters.');
    await expect(l.playlistCreate('x'.repeat(101), [])).rejects.toMatch(/1 to 100/);
    await expect(l.playlistRename(id, '')).rejects.toMatch(/1 to 100/);
    await expect(l.playlistRename(99, 'x')).rejects.toBe('That playlist no longer exists.');
  });

  it('adding skips songs already present and reports it', async () => {
    const l = new MockLibrary();
    const id = await l.playlistCreate('P', [t('a')]);
    expect(await l.playlistAdd(id, [t('a'), t('b'), t('b'), t('c')])).toEqual({ added: 2, skipped: 2 });
    expect(await titles(l, id)).toEqual(['Title a', 'Title b', 'Title c']);
  });

  it('deleting a playlist returns a snapshot that restores it exactly', async () => {
    const l = new MockLibrary();
    const id = await l.playlistCreate('Keep', [t('a'), t('b')]);
    const snap = await l.playlistDelete(id);
    await expect(l.playlistGet(id)).rejects.toMatch(/no longer exists/);
    await l.playlistRestore(snap);
    expect(await titles(l, id)).toEqual(['Title a', 'Title b']);
  });

  it('removing an entry can be restored in place, and moving works like dragging', async () => {
    const l = new MockLibrary();
    const id = await l.playlistCreate('P', [t('a'), t('b'), t('c'), t('d')]);
    const entries = (await l.playlistGet(id)).entries;
    const removed = await l.playlistRemoveEntry(entries[1]!.entryId);
    expect(removed.position).toBe(1);
    expect(await titles(l, id)).toEqual(['Title a', 'Title c', 'Title d']);
    await l.playlistRestoreEntry(removed);
    expect(await titles(l, id)).toEqual(['Title a', 'Title b', 'Title c', 'Title d']);
    await l.playlistMoveEntry((await l.playlistGet(id)).entries[0]!.entryId, 2);
    expect(await titles(l, id)).toEqual(['Title b', 'Title c', 'Title a', 'Title d']);
  });

  it('clearing history can be undone and keeps newest-first order', async () => {
    const l = new MockLibrary();
    l.recordPlay(t('a'));
    l.recordPlay(t('b'));
    const before = await l.historyGet(10);
    const cleared = await l.historyClear();
    expect((await l.getLibrary()).historyCount).toBe(0);
    await l.historyRestore(cleared);
    expect(await l.historyGet(10)).toEqual(before);
  });

  it('every change notifies subscribers with a higher revision', async () => {
    const l = new MockLibrary();
    const seen: number[] = [];
    const off = await l.onLibrary((s) => seen.push(s.revision));
    await l.setLiked(t('a'), true);
    await l.playlistCreate('P', []);
    off();
    await l.setLiked(t('b'), true);
    expect(seen.length).toBe(2);
    expect(seen[1]!).toBeGreaterThan(seen[0]!);
  });

  it('home favours songs you play and like', async () => {
    const l = new MockLibrary();
    for (let i = 0; i < 3; i++) l.recordPlay(t('often'));
    await l.setLiked(t('liked'), true);
    l.recordPlay(t('once'));
    const h = await l.homeGet();
    expect(h.quickPicks.map((x) => x.key)).toEqual(['yt:often', 'yt:liked', 'yt:once']);
    expect(h.recent.map((x) => x.key)).toEqual(['yt:once', 'yt:often']);
    expect(h.likedCount).toBe(1);
  });
});
