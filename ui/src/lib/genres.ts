import type { GenreDto } from './bindings/GenreDto';

/**
 * Genres to put in front, in this order, whatever order YouTube Music lists them in: Japanese and Korean pop and
 * the music around anime. The rest follow in the website's own order.
 */
const FIRST: RegExp[] = [/\banime\b/i, /\bj-?pop\b/i, /\bk-?pop\b/i, /\bj-?rock\b/i, /city\s*pop/i, /vocaloid/i, /\bc-?pop\b|mandopop|cantopop/i];

/** How many genres show before "More genres". */
export const GENRES_SHOWN = 12;

function rank(g: GenreDto): number {
  const i = FIRST.findIndex((re) => re.test(g.title));
  return i === -1 ? FIRST.length : i;
}

/** The favourites first (see `FIRST`), everything else after them in its original order. */
export function orderGenres(genres: GenreDto[]): GenreDto[] {
  return genres
    .map((g, i) => ({ g, i, r: rank(g) }))
    .sort((a, b) => a.r - b.r || a.i - b.i)
    .map((x) => x.g);
}
