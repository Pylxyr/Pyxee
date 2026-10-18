import type { Backend } from './backend';
import { toSearchError } from './backend';
import type { HistoryItemDto } from './bindings/HistoryItemDto';
import type { TrackDto } from './bindings/TrackDto';
import { freshTracks, jumpBackIn, pickSeeds, shelfTitle } from './explore';

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
 * Explore: "jump back in" from history, plus one shelf of similar songs per seed (likes first, then recent plays).
 * Shelves load one after another (yt-dlp is paced to one run at a time anyway) and an answer for an older load is dropped.
 */
export class ExploreStore {
  status = $state<'idle' | 'loading' | 'ready'>('idle');
  jump = $state<TrackDto[]>([]);
  shelves = $state<Shelf[]>([]);
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

    const seen = new Set<string>(seeds.map((s) => s.track.key));
    let blocked: string | null = null;
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
        shelf.tracks = freshTracks(related, seen);
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
}

export const explore = new ExploreStore();
