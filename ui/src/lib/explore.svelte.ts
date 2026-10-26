import type { Backend } from './backend';
import { toSearchError } from './backend';
import type { ExploreShelfDto } from './bindings/ExploreShelfDto';
import type { GenreDto } from './bindings/GenreDto';
import type { HistoryItemDto } from './bindings/HistoryItemDto';
import type { TrackDto } from './bindings/TrackDto';
import { freshTracks, jumpBackIn, pickSeeds, shelfTitle, withoutJunk, withoutJunkTracks } from './explore';
import { orderGenres } from './genres';

export interface Shelf {
  id: string;
  title: string;
  seed: TrackDto;
  status: 'loading' | 'ready' | 'error';
  tracks: TrackDto[];
  error: string | null;
}

/** How many related songs one shelf asks for. */
export const SHELF_SIZE = 12;
/** Failures that will not get better by asking again right away; the remaining shelves are not attempted. */
const STOP_ON = new Set(['offline', 'rateLimited', 'toolMissing']);

/**
 * Explore: "jump back in" from history, and one shelf of similar songs per seed (likes first, then recent plays). On top sit
 * YouTube Music's genres (K-Pop, J-Pop, Anime...): picking one swaps the page for that genre's own playlists and songs.
 * Requests go one after another (they share one pacing gate with search) and an answer for an older load is dropped.
 */
export class ExploreStore {
  status = $state<'idle' | 'loading' | 'ready'>('idle');
  jump = $state<TrackDto[]>([]);
  shelves = $state<Shelf[]>([]);
  /** YouTube Music's genres and moods, the favourites (anime, J-Pop, K-Pop) first. */
  genres = $state<GenreDto[]>([]);
  genresStatus = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  /** The genre being shown instead of the usual page; null is the usual page. */
  selectedGenre = $state<GenreDto | null>(null);
  genreShelves = $state<ExploreShelfDto[]>([]);
  genreStatus = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  genreError = $state<string | null>(null);
  #genreCache = new Map<string, ExploreShelfDto[]>();
  #genreSeq = 0;
  /** True once there was nothing to build shelves from. */
  noSeeds = $state(false);
  #seq = 0;
  #backend: Backend | null = null;

  /** Loads once per session; `refresh` loads again. */
  async ensure(backend: Backend) {
    if (this.status === 'idle') await this.load(backend);
  }

  async refresh() {
    if (this.#backend) await this.load(this.#backend);
  }

  async load(backend: Backend) {
    this.#backend = backend;
    const seq = ++this.#seq;
    this.#genreCache.clear(); // Refresh asks YouTube again, genres included
    this.status = 'loading';
    const [liked, history] = await Promise.all([
      backend.likedGet().catch((): TrackDto[] => []),
      backend.historyGet(60).catch((): HistoryItemDto[] => []),
    ]);
    if (seq !== this.#seq) return;
    this.jump = jumpBackIn(history);
    const seeds = pickSeeds(liked, history);
    this.noSeeds = seeds.length === 0;
    this.shelves = seeds.map((s) => ({
      id: s.track.key,
      title: shelfTitle(s),
      seed: s.track,
      status: 'loading',
      tracks: [],
      error: null,
    }));
    this.status = 'ready';

    // The genre chips come first: one request, and they are the top of what the listener sees.
    let blocked: string | null = null;
    if (!blocked && this.genresStatus !== 'ready') await this.loadGenres(backend);
    if (seq !== this.#seq) return;
    if (this.selectedGenre) void this.selectGenre(backend, this.selectedGenre);

    const seen = new Set<string>(seeds.map((s) => s.track.key));
    for (const shelf of this.shelves) {
      if (seq !== this.#seq) return;
      if (blocked) {
        shelf.status = 'error';
        shelf.error = blocked;
        continue;
      }
      try {
        const related = await backend.relatedGet(shelf.seed.key, SHELF_SIZE);
        if (seq !== this.#seq) return;
        shelf.tracks = freshTracks(withoutJunkTracks(related), seen);
        shelf.status = 'ready';
      } catch (e) {
        if (seq !== this.#seq) return;
        const err = toSearchError(e);
        shelf.status = 'error';
        shelf.error = err.message;
        if (STOP_ON.has(err.kind)) blocked = err.message;
      }
    }
  }

  /** The genre list, once; a failure leaves the usual page as it is (the chips just do not show). */
  async loadGenres(backend: Backend) {
    this.#backend = backend;
    this.genresStatus = 'loading';
    try {
      const list = await backend.genresGet();
      this.genres = orderGenres(list);
      this.genresStatus = 'ready';
    } catch {
      this.genresStatus = 'error';
    }
  }

  /** Shows a genre's own shelves (asked once per session, then remembered); `null` goes back to the usual page. */
  async selectGenre(backend: Backend, genre: GenreDto | null) {
    const seq = ++this.#genreSeq;
    this.selectedGenre = genre;
    this.genreError = null;
    if (!genre) {
      this.genreShelves = [];
      this.genreStatus = 'idle';
      return;
    }
    const known = this.#genreCache.get(genre.params);
    if (known) {
      this.genreShelves = known;
      this.genreStatus = 'ready';
      return;
    }
    this.genreShelves = [];
    this.genreStatus = 'loading';
    try {
      const shelves = await backend.genreGet(genre.params);
      if (seq !== this.#genreSeq) return;
      const clean = withoutJunk(shelves);
      this.#genreCache.set(genre.params, clean);
      this.genreShelves = clean;
      this.genreStatus = 'ready';
    } catch (e) {
      if (seq !== this.#genreSeq) return;
      this.genreStatus = 'error';
      this.genreError = toSearchError(e).message;
    }
  }

  /** Asks the genre again, after a failure. */
  async retryGenre(backend: Backend) {
    const genre = this.selectedGenre;
    if (genre) {
      this.#genreCache.delete(genre.params);
      await this.selectGenre(backend, genre);
    }
  }
}

export const explore = new ExploreStore();
