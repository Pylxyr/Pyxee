import { describe, expect, it } from 'vitest';
import type { HistoryItemDto } from './bindings/HistoryItemDto';
import type { TrackDto } from './bindings/TrackDto';
import type { ExploreShelfDto } from './bindings/ExploreShelfDto';
import { freshTracks, isJunk, jumpBackIn, pickSeeds, shelfTitle, withoutJunk } from './explore';

const t = (n: number, artist = `Artist ${n}`): TrackDto => ({ key: `k${n}`, title: `Song ${n}`, artist, durationMs: 1000, thumbnail: null });
const h = (track: TrackDto, playedAt = 1): HistoryItemDto => ({ track, playedAt });

describe('pickSeeds', () => {
  it('prefers likes, then plays, and never repeats an artist or a song', () => {
    const seeds = pickSeeds([t(1, 'Same'), t(2, 'same')], [h(t(3)), h(t(1, 'Other')), h(t(4))], 3);
    expect(seeds.map((s) => [s.track.key, s.reason])).toEqual([['k1', 'liked'], ['k3', 'played'], ['k4', 'played']]);
  });
  it('is empty when there is nothing to learn from', () => {
    expect(pickSeeds([], [])).toEqual([]);
  });
  it('stops at the maximum', () => {
    expect(pickSeeds([t(1), t(2), t(3), t(4)], [], 2)).toHaveLength(2);
  });
});

describe('shelf helpers', () => {
  it('titles say why a shelf exists', () => {
    expect(shelfTitle({ track: t(1), reason: 'liked' })).toBe('Because you liked Song 1');
    expect(shelfTitle({ track: t(1), reason: 'played' })).toBe('Because you played Song 1');
  });
  it('jump back in lists each recent song once, newest first, up to the limit', () => {
    expect(jumpBackIn([h(t(1)), h(t(2)), h(t(1)), h(t(3))], 2).map((x) => x.key)).toEqual(['k1', 'k2']);
  });
  it('fresh tracks drop what another shelf or the seed already shows', () => {
    const seen = new Set(['k1']);
    expect(freshTracks([t(1), t(2), t(2), t(3)], seen).map((x) => x.key)).toEqual(['k2', 'k3']);
    expect(freshTracks([t(3), t(4)], seen).map((x) => x.key)).toEqual(['k4']);
  });
});

describe('junk', () => {
  it('drops mashups, AI-made songs, study loops and hour-long compilations', () => {
    for (const title of ['New Tiktok Mashup 2026', 'Lofi Beats for Studying', 'Song Name (AI Cover)', 'Made with Suno', 'Rain sounds 10 hours', 'Best of 2026 Mega Medley']) {
      expect(isJunk(title), title).toBe(true);
    }
    expect(isJunk('A long one', 'Someone', 20 * 60 * 1000)).toBe(true);
  });

  it('keeps ordinary songs, including an artist called Ai and a song called Nonstop', () => {
    expect(isJunk('Neon Harbor', 'Orbit')).toBe(false);
    expect(isJunk('Story', 'Ai')).toBe(false);
    expect(isJunk('Nonstop', 'Drake', 239_000)).toBe(false);
  });

  it('removes junk from every kind of shelf and drops a shelf that ends up empty', () => {
    const page: ExploreShelfDto[] = [
      { kind: 'songs', id: 's', title: 'Trending', tracks: [{ key: 'a', title: 'Fine song', artist: 'X', durationMs: 200_000, thumbnail: null }, { key: 'b', title: 'Tiktok Mashup', artist: 'Y', durationMs: 200_000, thumbnail: null }] },
      { kind: 'albums', id: 'al', title: 'New', albums: [{ id: '1', title: 'Lofi Beats for Studying', artist: 'Z', thumbnail: null }] },
      { kind: 'playlists', id: 'p', title: 'Picks', playlists: [{ id: '2', title: 'Tiktok Mashup Hits', subtitle: 'Playlist', thumbnail: null }] },
    ];
    const out = withoutJunk(page);
    expect(out.map((s) => s.id)).toEqual(['s']);
    expect(out[0]).toMatchObject({ kind: 'songs', tracks: [{ key: 'a' }] });
  });
});
