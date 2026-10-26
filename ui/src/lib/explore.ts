import type { AlbumDto } from './bindings/AlbumDto';
import type { ExploreShelfDto } from './bindings/ExploreShelfDto';
import type { HistoryItemDto } from './bindings/HistoryItemDto';
import type { TrackDto } from './bindings/TrackDto';

/** A song Explore builds a shelf around, and why it was picked. */
export interface Seed {
  track: TrackDto;
  reason: 'liked' | 'played';
}

/**
 * What to search for to find an album or single on YouTube Music's Explore page. Pyxee has no album page, so a cover
 * opens an ordinary songs search for it: the release's own songs come first.
 */
export function albumQuery(album: AlbumDto): string {
  const title = album.title.trim();
  const artist = album.artist.trim();
  return artist && artist !== 'Unknown artist' ? `${title} ${artist}` : title;
}

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

/**
 * Mashups, AI-made songs and covers, and hour-long compilations are not what Explore is for. There is no way to know
 * for sure from a title, so this is a list of the plain giveaways (the word "mashup", "AI generated", Suno or Udio,
 * "beats to study to", "10 hours"), plus anything over 15 minutes. A bare "AI" is not matched: it is also an artist's name.
 */
const JUNK_TEXT: RegExp[] = [
  /\bmash[\s-]?ups?\b/i,
  /\bmedley\b|\bmegamix\b/i,
  /\btik[\s-]?tok\b/i,
  /\bai[\s-]?(?:generated|made|cover|covers|music|song|songs|voice|remix|version)\b|\bgenerated (?:by|with) ai\b|[(\[]\s*ai\s*[)\]]/i,
  /\bsuno\b|\budio\b|\bvoice (?:model|clone)\b/i,
  /\b(?:lo-?fi|chill|jazz|piano)?\s*(?:beats|music) (?:to|for) (?:study|studying|relax|relaxing|sleep|sleeping|work|working|focus|chill)\b|\b(?:study|sleep|relaxing|focus) music\b/i,
  /\b\d+\s*(?:hours?|hrs?)\b|\bfull album\b|\bplaylist\b/i,
];
const TOO_LONG_MS = 15 * 60 * 1000;

export function isJunk(title: string, artist = '', durationMs: number | null = null): boolean {
  if (durationMs !== null && durationMs >= TOO_LONG_MS) return true;
  return JUNK_TEXT.some((re) => re.test(title) || re.test(artist));
}

export function withoutJunkTracks(tracks: TrackDto[]): TrackDto[] {
  return tracks.filter((t) => !isJunk(t.title, t.artist, t.durationMs));
}

/** Explore's shelves without the junk; a shelf left with nothing in it goes too. */
export function withoutJunk(shelves: ExploreShelfDto[]): ExploreShelfDto[] {
  const out: ExploreShelfDto[] = [];
  for (const shelf of shelves) {
    if (shelf.kind === 'songs') {
      const tracks = withoutJunkTracks(shelf.tracks);
      if (tracks.length > 0) out.push({ ...shelf, tracks });
    } else if (shelf.kind === 'albums') {
      const albums = shelf.albums.filter((a) => !isJunk(a.title, a.artist));
      if (albums.length > 0) out.push({ ...shelf, albums });
    } else {
      const playlists = shelf.playlists.filter((p) => !isJunk(p.title));
      if (playlists.length > 0) out.push({ ...shelf, playlists });
    }
  }
  return out;
}
