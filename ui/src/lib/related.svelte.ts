import type { Backend } from './backend';
import { toSearchError } from './backend';
import type { SearchErrorDto } from './bindings/SearchErrorDto';
import type { TrackDto } from './bindings/TrackDto';

/** How many related songs the tab asks for. */
export const RELATED_SIZE = 20;
/** Wait this long on one song before asking, so skipping through the queue does not start a search per song. */
export const RELATED_DELAY_MS = 600;

/** Failures that will not get better by asking again right away. */
const BLOCKING = new Set(['offline', 'rateLimited', 'toolMissing']);
/** After one of those, other songs show the same message instead of asking, for this long (or until Try again). */
export const RELATED_BLOCK_MS = 30_000;

export type RelatedState =
  | { kind: 'idle' }
  | { kind: 'loading'; key: string }
  | { kind: 'ready'; key: string; tracks: TrackDto[] }
  | { kind: 'error'; key: string; error: SearchErrorDto };

/**
 * Songs similar to the one being shown (Now Playing > Related).
 * One request at a time, the answer for a song the user has already left is dropped, good answers are kept for the
 * session, and a failure is not retried until the user asks (so an offline or rate-limited service is not hammered).
 */
export class RelatedStore {
  state = $state<RelatedState>({ kind: 'idle' });
  #backend: Backend | null = null;
  #wanted: string | null = null;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #cache = new Map<string, TrackDto[]>();
  #blocked: { error: SearchErrorDto; until: number } | null = null;

  init(backend: Backend) {
    this.#backend = backend;
    this.#cache.clear();
    this.#blocked = null;
    this.#wanted = null;
    this.#clearTimer();
    this.state = { kind: 'idle' };
  }

  /** Looks the song up once (cached; a failed one waits for `retry`). Pass `null` when nothing is playing. */
  load(track: TrackDto | null, force = false) {
    if (!track) {
      this.#clearTimer();
      this.#wanted = null;
      this.state = { kind: 'idle' };
      return;
    }
    const key = track.key;
    const cached = this.#cache.get(key);
    if (cached) {
      this.#clearTimer();
      this.#wanted = key;
      this.state = { kind: 'ready', key, tracks: cached };
      return;
    }
    if (!force && this.#wanted === key && this.state.kind !== 'idle') return;
    if (!this.#backend) return;
    this.#clearTimer();
    this.#wanted = key;
    if (force) this.#blocked = null;
    if (this.#blocked && Date.now() < this.#blocked.until) {
      this.state = { kind: 'error', key, error: this.#blocked.error };
      return;
    }
    this.state = { kind: 'loading', key };
    const backend = this.#backend;
    this.#timer = setTimeout(() => {
      this.#timer = null;
      void this.#fetch(backend, key);
    }, force ? 0 : RELATED_DELAY_MS);
  }

  retry(track: TrackDto | null) {
    this.load(track, true);
  }

  async #fetch(backend: Backend, key: string) {
    try {
      const found = await backend.relatedGet(key, RELATED_SIZE);
      const seen = new Set<string>([key]);
      const tracks = found.filter((t) => !seen.has(t.key) && seen.add(t.key));
      this.#cache.set(key, tracks);
      this.#blocked = null;
      if (this.#wanted === key) this.state = { kind: 'ready', key, tracks };
    } catch (e) {
      const error = toSearchError(e);
      if (BLOCKING.has(error.kind)) this.#blocked = { error, until: Date.now() + RELATED_BLOCK_MS };
      if (this.#wanted === key) this.state = { kind: 'error', key, error };
    }
  }

  #clearTimer() {
    if (this.#timer) clearTimeout(this.#timer);
    this.#timer = null;
  }
}

export const relatedStore = new RelatedStore();
