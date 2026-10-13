import type { Backend } from './backend';
import type { LyricsDto } from './bindings/LyricsDto';
import type { TrackDto } from './bindings/TrackDto';

export type LyricsState =
  | { kind: 'idle' }
  | { kind: 'loading'; key: string }
  | { kind: 'ready'; key: string; lyrics: LyricsDto };

/** Lyrics of the song being shown. A late answer for a song the user has already left is dropped. */
export class LyricsStore {
  state = $state<LyricsState>({ kind: 'idle' });
  #backend: Backend | null = null;
  #wanted: string | null = null;

  init(backend: Backend) {
    this.#backend = backend;
  }

  /** Looks the song up (once per song until `retry`). Pass `null` when nothing is playing. */
  async load(track: TrackDto | null, force = false) {
    if (!track) {
      this.#wanted = null;
      this.state = { kind: 'idle' };
      return;
    }
    if (!force && this.#wanted === track.key && this.state.kind !== 'idle') return;
    if (!this.#backend) return;
    this.#wanted = track.key;
    this.state = { kind: 'loading', key: track.key };
    const lyrics = await this.#backend.lyricsGet(track);
    if (this.#wanted !== track.key) return; // the song changed while we waited
    this.state = { kind: 'ready', key: track.key, lyrics };
  }

  retry(track: TrackDto | null) {
    return this.load(track, true);
  }
}

export const lyricsStore = new LyricsStore();
