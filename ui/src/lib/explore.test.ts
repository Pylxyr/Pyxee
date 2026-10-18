import { describe, expect, it } from 'vitest';
import type { HistoryItemDto } from './bindings/HistoryItemDto';
import type { TrackDto } from './bindings/TrackDto';
import { MOODS, freshTracks, jumpBackIn, pickSeeds, shelfTitle } from './explore';

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
  it('moods have unique ids and non-empty queries', () => {
    expect(new Set(MOODS.map((m) => m.id)).size).toBe(MOODS.length);
    for (const m of MOODS) expect(m.query.trim().length).toBeGreaterThan(2);
  });
});
