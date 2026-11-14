import { describe, expect, it } from 'vitest';
import { orderGenres } from './genres';

const g = (title: string) => ({ title, params: `p-${title}` });
const titles = (list: { title: string }[]) => list.map((x) => x.title);

describe('orderGenres', () => {
  it('puts anime, J-Pop and K-Pop first and keeps the website order for the rest', () => {
    const listed = [g('Chill'), g('Rock'), g('K-Pop'), g('Hip-Hop'), g('J-Pop'), g('Anime'), g('Jazz')];
    expect(titles(orderGenres(listed))).toEqual(['Anime', 'J-Pop', 'K-Pop', 'Chill', 'Rock', 'Hip-Hop', 'Jazz']);
  });

  it('recognises the usual spellings and leaves the input alone', () => {
    const listed = [g('Pop'), g('JPop'), g('Kpop'), g('City Pop'), g('Anime & Game')];
    const before = [...listed];
    expect(titles(orderGenres(listed))).toEqual(['Anime & Game', 'JPop', 'Kpop', 'City Pop', 'Pop']);
    expect(listed).toEqual(before);
  });

  it('does not take a word that merely contains a name for it', () => {
    expect(titles(orderGenres([g('Animated movies'), g('Anime')]))).toEqual(['Anime', 'Animated movies']);
  });
});
