import type { HistoryItemDto } from './bindings/HistoryItemDto';
import type { TrackDto } from './bindings/TrackDto';

/** A song Explore builds a shelf around, and why it was picked. */
export interface Seed {
  track: TrackDto;
  reason: 'liked' | 'played';
}

/** Quick searches in a tone of voice. Each one opens an ordinary search, so nothing here needs its own backend. */
export const MOODS: { id: string; label: string; query: string }[] = [
  { id: 'focus', label: 'Focus', query: 'focus instrumental' },
  { id: 'chill', label: 'Chill', query: 'chill vibes' },
  { id: 'workout', label: 'Workout', query: 'workout energy' },
  { id: 'late', label: 'Late night', query: 'late night drive' },
  { id: 'morning', label: 'Slow morning', query: 'acoustic morning' },
  { id: 'throwback', label: 'Throwback', query: '2000s hits' },
  { id: 'rain', label: 'Rainy day', query: 'rainy day mellow' },
  { id: 'party', label: 'Party', query: 'party dance' },
];

const norm = (s: string) => s.trim().toLowerCase();

/**
 * Up to `max` songs to build shelves around: likes first, then recent plays, never two from the same artist,
 * so three shelves do not all say the same thing.
 */
export function pickSeeds(liked: TrackDto[], history: HistoryItemDto[], max = 3): Seed[] {
  const seeds: Seed[] = [];
  const artists = new Set<string>();
  const keys = new Set<string>();
  const consider = (track: TrackDto, reason: Seed['reason']) => {
    if (seeds.length >= max || keys.has(track.key) || artists.has(norm(track.artist))) return;
    keys.add(track.key);
    artists.add(norm(track.artist));
    seeds.push({ track, reason });
  };
  for (const t of liked) consider(t, 'liked');
  for (const h of history) consider(h.track, 'played');
  return seeds;
}

export function shelfTitle(seed: Seed): string {
  return seed.reason === 'liked' ? `Because you liked ${seed.track.title}` : `Because you played ${seed.track.title}`;
}

/** Recent songs, each once, newest first. */
export function jumpBackIn(history: HistoryItemDto[], max = 10): TrackDto[] {
  const seen = new Set<string>();
  const out: TrackDto[] = [];
  for (const h of history) {
    if (seen.has(h.track.key)) continue;
    seen.add(h.track.key);
    out.push(h.track);
    if (out.length >= max) break;
  }
  return out;
}

/** Drops songs an earlier shelf (or the seed itself) already shows, and remembers the rest. */
export function freshTracks(tracks: TrackDto[], seen: Set<string>): TrackDto[] {
  const out: TrackDto[] = [];
  for (const t of tracks) {
    if (seen.has(t.key)) continue;
    seen.add(t.key);
    out.push(t);
  }
  return out;
}
