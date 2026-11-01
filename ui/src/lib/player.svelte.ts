import type { Backend } from './backend';
import { perfCounters } from './perf';
import type { CommandDto } from './bindings/CommandDto';
import type { EntryDto } from './bindings/EntryDto';
import type { FailureDto } from './bindings/FailureDto';
import type { PlayerStateDto } from './bindings/PlayerStateDto';
import type { RepeatDto } from './bindings/RepeatDto';
import type { TrackDto } from './bindings/TrackDto';

/** A position report older than this is not extended by the clock: reports have stopped (the window was asleep). */
export const STALE_AFTER_MS = 1500;

export const EMPTY_STATE: PlayerStateDto = {
  seq: 0, transport: { state: 'idle' }, audible: null, loading: null, positionMs: 0, repeat: 'off',
  shuffle: false, autoplay: true, canUndo: false, queue: [], current: null, upcoming: [],
};

export function describeFailure(f: FailureDto): string {
  switch (f.kind) {
    case 'offline': return "You're offline. Check your connection and try again.";
    case 'rateLimited': return 'Too many requests. Wait a moment and try again.';
    case 'unavailable': return "This track isn't available.";
    case 'decode': return "This track couldn't be played.";
    case 'expired': return f.status ? `YouTube refused to send it (HTTP ${f.status}).` : 'YouTube refused to send it.';
    case 'other': return f.message;
  }
}

const NEXT_REPEAT: Record<RepeatDto, RepeatDto> = { off: 'all', all: 'one', one: 'off' };

/** UI-side view of the Rust player. State is only ever replaced by what the backend pushes. */
export class PlayerStore {
  /**
   * What the backend last pushed as a whole (queue, transport, ...). Replaced only by a full state, never by a
   * position-only push: a value that is replaced four times a second would make everything that reads anything from
   * it (the queue, the up-next list, the shown song) run again four times a second. `positionMs` and `seq` inside
   * it are those of the last full state: read the position from `positionMs` below.
   */
  #state = $state.raw<PlayerStateDto>(EMPTY_STATE);
  /** Where the song is, apart from the rest so that only what shows the position follows the 4 Hz pushes. */
  positionMs = $state(0);
  /** The newest `seq` seen, from a full state or a position-only push. */
  #seq = 0;
  /** When `positionMs` was last set: what lets a lyric line change at its own time between the (1 Hz) reports. */
  #positionAt = Date.now();
  ready = $state(false);
  catalog = $state<TrackDto[] | null>(null);
  #backend: Backend | null = null;
  #unlisten: (() => void) | null = null;
  /** The window may have been asleep (hidden in the tray) while pushes went by: ask again when it wakes. */
  get state(): PlayerStateDto {
    return this.#state;
  }
  /** A whole state: it also sets the position and the ordering counter. (Only the backend's pushes, and tests.) */
  set state(s: PlayerStateDto) {
    this.#state = s;
    this.positionMs = s.positionMs;
    this.#positionAt = Date.now();
    this.#seq = s.seq;
  }
  /** How long ago the position was last reported. */
  positionAgeMs(): number {
    return Math.max(0, Date.now() - this.#positionAt);
  }
  /** Where the song is now: the last report plus the time since it while playing (never more than a report's gap). */
  estimatedPositionMs(): number {
    return this.positionMs + (this.isPlaying ? Math.min(this.positionAgeMs(), STALE_AFTER_MS) : 0);
  }
  #wake = () => {
    if (!document.hidden) void this.resync();
  };

  /** What the user sees as the current track: audible, else loading, else the restored selection. */
  shown = $derived(this.entry(this.state.audible) ?? this.entry(this.state.loading) ?? this.entry(this.state.current));
  isPlaying = $derived(this.state.transport.state === 'playing');
  isBusy = $derived(this.state.transport.state === 'loading' || this.state.transport.state === 'stalled');
  failure = $derived(this.state.transport.state === 'failed' ? this.state.transport.failure : null);
  durationMs = $derived(this.shown?.track.durationMs ?? null);
  upNext = $derived(this.state.upcoming.map((id) => this.entry(id)).filter((e): e is EntryDto => e != null));

  entry(id: number | null): EntryDto | null {
    return id == null ? null : (this.state.queue.find((e) => e.id === id) ?? null);
  }

  async init(backend: Backend) {
    this.#unlisten?.();
    this.#backend = backend;
    this.state = EMPTY_STATE; // a new backend counts its states from the start
    const stopState = await backend.onState((s) => {
      perfCounters.state++;
      this.#apply(s);
    });
    const stopPosition = await backend.onPosition((p) => {
      perfCounters.position++;
      if (p.seq >= this.#seq) {
        this.#seq = p.seq;
        this.positionMs = p.positionMs;
        this.#positionAt = Date.now();
      }
    });
    this.#unlisten = () => {
      stopState();
      stopPosition();
    };
    if (typeof document !== 'undefined') {
      document.removeEventListener('visibilitychange', this.#wake);
      document.addEventListener('visibilitychange', this.#wake);
    }
    // Not a plain assignment: a state pushed while this answer was on its way is newer than the answer.
    this.#apply(await backend.getState());
    this.ready = true;
    this.catalog = await backend.devCatalog().catch(() => []);
  }

  /**
   * Keeps the newest state. Two threads publish, and a slow answer to `getState` can land after a push, so an arrival
   * is not necessarily the latest: the highest `seq` wins and an older one is dropped.
   */
  #apply(s: PlayerStateDto) {
    if (s.seq >= this.#seq) this.state = s;
  }

  /** Takes the backend's state again (what is shown stays if it cannot be read). */
  async resync() {
    try {
      const s = await this.#backend?.getState();
      if (s) this.#apply(s);
    } catch {
      /* keep what is shown; the next push replaces it */
    }
  }

  dispatch(command: CommandDto): Promise<void> {
    return this.#backend?.dispatch(command) ?? Promise.resolve();
  }

  setVolume = (percent: number): Promise<void> => this.#backend?.setVolume(percent) ?? Promise.resolve();
  toggle = () => this.dispatch({ type: 'toggle' });
  next = () => this.dispatch({ type: 'next' });
  previous = () => this.dispatch({ type: 'previous' });
  seek = (ms: number) => this.dispatch({ type: 'seek', ms: Math.max(0, Math.round(ms)) });
  retry = () => this.dispatch({ type: 'retry' });
  toggleShuffle = () => this.dispatch({ type: 'setShuffle', on: !this.state.shuffle });
  cycleRepeat = () => this.dispatch({ type: 'setRepeat', repeat: NEXT_REPEAT[this.state.repeat] });
  playTracks = (tracks: TrackDto[], start = 0) => this.dispatch({ type: 'playTracks', tracks, start });
  playEntry = (id: number) => this.dispatch({ type: 'playEntry', id });
  playNext = (track: TrackDto) => this.dispatch({ type: 'playNext', track });
  addToQueue = (track: TrackDto) => this.dispatch({ type: 'addToQueue', track });
  remove = (id: number) => this.dispatch({ type: 'remove', id });
  /** Reorders Up next; `to` counts among the upcoming songs (0 = plays next). */
  moveUpcoming = (id: number, to: number) => this.dispatch({ type: 'moveUpcoming', id, to });
  undo = () => this.dispatch({ type: 'undo' });
}

export const player = new PlayerStore();
