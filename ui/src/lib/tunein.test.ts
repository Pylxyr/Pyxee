import { describe, expect, it } from 'vitest';
import type { TrackDto } from './bindings/TrackDto';
import { tuneInSeeds } from './tunein';

const song = (key: string, artist: string): TrackDto => ({ key, title: key, artist, durationMs: 1000, thumbnail: null });
const keys = (l: TrackDto[]) => l.map((t) => t.key);

describe('the songs Tune in offers', () => {
  it('lists the quick picks first, then the latest plays, each song once', () => {
    const out = tuneInSeeds([song('a', 'A'), song('b', 'B')], [song('b', 'B'), song('c', 'C')]);
    expect(keys(out)).toEqual(['a', 'b', 'c']);
  });
  it('leaves out the song that is playing now', () => {
    const out = tuneInSeeds([song('a', 'A'), song('b', 'B')], [], 'a');
    expect(keys(out)).toEqual(['b']);
  });
  it('shows one song per artist, whatever the case of the name, while there are other artists', () => {
    const quick = [song('a1', 'Mix'), song('a2', 'mix'), song('a3', 'Mix'), ...'bcdefg'.split('').map((c) => song(c, c.toUpperCase()))];
    const out = tuneInSeeds(quick, []);
    expect(keys(out)).toEqual(['a1', 'b', 'c', 'd', 'e', 'f', 'g']);
  });
  it('gives a small library a short row: one card per artist, never a second one of the same artist', () => {
    const quick = [song('a1', 'A'), song('a2', 'A'), song('b1', 'B'), song('a3', 'A'), song('b2', 'B')];
    expect(keys(tuneInSeeds(quick, []))).toEqual(['a1', 'b1']);
    expect(keys(tuneInSeeds([song('z1', 'ZUTOMAYO'), song('z2', 'ZUTOMAYO')], [song('z3', 'zutomayo')]))).toEqual(['z1']);
  });
  it('never offers more than the row holds', () => {
    const many = Array.from({ length: 30 }, (_, i) => song(`s${i}`, `Artist ${i}`));
    expect(tuneInSeeds(many, [])).toHaveLength(11);
    expect(tuneInSeeds(many, [], null, 4)).toHaveLength(4);
  });
  it('has nothing to offer for an empty library', () => {
    expect(tuneInSeeds([], [])).toEqual([]);
  });
});
