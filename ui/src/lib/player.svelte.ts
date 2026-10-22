import type { Backend } from './backend';
import type { CommandDto } from './bindings/CommandDto';
import type { EntryDto } from './bindings/EntryDto';
import type { FailureDto } from './bindings/FailureDto';
import type { PlayerStateDto } from './bindings/PlayerStateDto';
import type { RepeatDto } from './bindings/RepeatDto';
import type { TrackDto } from './bindings/TrackDto';

export const EMPTY_STATE: PlayerStateDto = {
  transport: { state: 'idle' }, audible: null, loading: null, positionMs: 0, repeat: 'off',
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
  state = $state<PlayerStateDto>(EMPTY_STATE);
  ready = $state(false);
  catalog = $state<TrackDto[] | null>(null);
  #backend: Backend | null = null;
  #unlisten: (() => void) | null = null;
  /** The window may have been asleep (hidden in the tray) while pushes went by: ask again when it wakes. */
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
    this.#unlisten = await backend.onState((s) => (this.state = s));
    if (typeof document !== 'undefined') {
      document.removeEventListener('visibilitychange', this.#wake);
      document.addEventListener('visibilitychange', this.#wake);
    }
    this.state = await backend.getState();
    this.ready = true;
    this.catalog = await backend.devCatalog().catch(() => []);
  }

  /** Takes the backend's state again (what is shown stays if it cannot be read). */
  async resync() {
    try {
      const s = await this.#backend?.getState();
      if (s) this.state = s;
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
