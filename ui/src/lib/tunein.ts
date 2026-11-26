import type { TrackDto } from './bindings/TrackDto';
import { artistOf } from './format';

/** The cards on Home's "Tune in" row, besides the "Surprise me" one. */
export const TUNE_IN_CARDS = 11;

/**
 * The songs "Tune in" offers as the start of a radio: what you come back to (the quick picks, most wanted first) that
 * "Recently played" is not already showing right below, so the two rows are two different things: a favourite you have
 * not played in a while is here, the song you just played is there. A song that is playing right now is left out (its
 * radio is already on), a song is shown once, and so is an artist (a card is titled with the artist, so two cards of
 * one artist would read the same). A library with little history simply gets a short row, or none.
 */
export function tuneInSeeds(quickPicks: TrackDto[], recent: TrackDto[], playing: string | null = null, max = TUNE_IN_CARDS): TrackDto[] {
  const seen = new Set<string>(recent.map((t) => t.key));
  const pool: TrackDto[] = [];
  for (const t of quickPicks) {
    if (t.key === playing || seen.has(t.key)) continue;
    seen.add(t.key);
    pool.push(t);
  }
  const artists = new Set<string>();
  const out: TrackDto[] = [];
  for (const t of pool) {
    const who = artistOf(t).toLowerCase();
    if (artists.has(who)) continue;
    artists.add(who);
    out.push(t);
    if (out.length >= max) return out;
  }
  return out;
}
