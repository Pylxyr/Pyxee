import type { Backend } from './backend';
import { MockLibrary } from './mock-library';
import type { CommandDto } from './bindings/CommandDto';
import type { EntryDto } from './bindings/EntryDto';
import type { PlayerStateDto } from './bindings/PlayerStateDto';
import type { TrackDto } from './bindings/TrackDto';
import type { WindowInfoDto } from './bindings/WindowInfoDto';
import type { FilterInfoDto } from './bindings/FilterInfoDto';
import type { SearchFilterDto } from './bindings/SearchFilterDto';
import type { SearchItemDto } from './bindings/SearchItemDto';
import type { SearchResponseDto } from './bindings/SearchResponseDto';

/** JSON round trip, like the real IPC boundary. (structuredClone rejects Svelte's reactive proxies.) */
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T;

export const DEMO_TRACKS: TrackDto[] = [
  ['Neon Harbor', 'Lowtide', 38],
  ['Paper Planes', 'Alma Vey', 31],
  ['Slow Orbit', 'Kestrel Hall', 36],
  ['Golden Hour Static', 'Marlow', 29],
  ['Glass Rooms', 'Nova Fen', 34],
  ['Afterimage', 'Dune Choir', 40],
  ['Low Light Parade', 'Wren & Co.', 27],
  ['Salt and Signal', 'Harbor Lines', 33],
].map(([title, artist, secs], i) => ({
  key: `demo:${i}`,
  title: title as string,
  artist: artist as string,
  durationMs: (secs as number) * 1000,
  thumbnail: null,
}));

/**
 * Browser/test stand-in for the Rust backend. It models just enough of the real state machine
 * (loading -> audible, gapless advance, queue edits, undo) to develop and test the UI without Tauri.
 * The real behaviour lives in `pyxee-core`; do not add features here that core does not have.
 */
export class MockBackend extends MockLibrary implements Backend {
  #s: PlayerStateDto = {
    transport: { state: 'idle' }, audible: null, loading: null, positionMs: 0, repeat: 'off',
    shuffle: false, autoplay: true, canUndo: false, queue: [], current: null, upcoming: [],
  };
  #next = 1;
  #listeners = new Set<(s: PlayerStateDto) => void>();
  #loadMs = 0;
  #undo: ({ entry: EntryDto; index: number } | { cleared: EntryDto[]; current: number | null })[] = [];
  #timer: ReturnType<typeof setInterval> | null = null;
  /** Simulated search latency (ms); tests set it to 0. */
  searchLatencyMs = 350;
  #listenedMs = 0;
  #counted = false;

  /** A new listen begins (also when a repeated song starts over). */
  #newListen() {
    this.#listenedMs = 0;
    this.#counted = false;
  }

  start(tickMs = 250) {
    this.#timer ??= setInterval(() => this.tick(tickMs), tickMs);
  }
  stop() {
    if (this.#timer) clearInterval(this.#timer);
    this.#timer = null;
  }

  async getState() { return clone(this.#s); }
  async windowInfo(): Promise<WindowInfoDto> { return { backdrop: 'none' }; }
  /** The last window theme the UI asked for (`null` = follow the system), so tests can check it arrived. */
  windowTheme: 'dark' | 'light' | null | undefined = undefined;
  async setWindowTheme(theme: 'dark' | 'light' | null) { this.windowTheme = theme; }
  /** The last volume the UI asked for (0-100), so tests can check it arrived. */
  volume = 100;
  async setVolume(percent: number) { this.volume = percent; }
  /** Demo log location, and a count of how many times the folder was opened. */
  logFolderOpens = 0;
  logError: string | null = null;
  async logInfo() {
    return {
      dir: 'C:\\Users\\you\\AppData\\Roaming\\dev.pyxee.app\\logs',
      file: 'C:\\Users\\you\\AppData\\Roaming\\dev.pyxee.app\\logs\\pyxee.log',
      error: this.logError,
    };
  }
  async openLogFolder() { this.logFolderOpens++; }
  #noticeListeners = new Set<(m: string) => void>();
  async onNotice(cb: (m: string) => void) {
    this.#noticeListeners.add(cb);
    return () => void this.#noticeListeners.delete(cb);
  }
  /** Simulates the backend telling the listener something (e.g. a song was skipped). */
  emitNotice(message: string) {
    for (const cb of this.#noticeListeners) cb(message);
  }
  async devCatalog() { return DEMO_TRACKS; }
  async searchFilters(): Promise<FilterInfoDto[]> {
    return (['all', 'songs', 'videos', 'albums', 'artists', 'playlists'] as SearchFilterDto[]).map((filter) => ({
      filter,
      supported: filter !== 'albums',
    }));
  }
  async suggest(prefix: string) {
    const p = prefix.trim().toLowerCase();
    if (!p) return [];
    return DEMO_TRACKS.flatMap((t) => [t.title, t.artist]).filter((s, i, a) => s.toLowerCase().includes(p) && a.indexOf(s) === i).slice(0, 6);
  }
  /** Demo search. Queries starting with !offline, !ratelimit or !missing simulate those failures. */
  async search(query: string, filter: SearchFilterDto): Promise<SearchResponseDto> {
    await new Promise((r) => setTimeout(r, this.searchLatencyMs));
    const q = query.trim().toLowerCase();
    if (q.startsWith('!offline')) throw { kind: 'offline', message: "You're offline, or the music service can't be reached." };
    if (q.startsWith('!ratelimit')) throw { kind: 'rateLimited', message: 'The music service is limiting requests right now. Try again in a moment.' };
    if (q.startsWith('!missing')) throw { kind: 'toolMissing', message: 'yt-dlp was not found. Check that it is installed or set its path.' };
    if (filter === 'albums') throw { kind: 'unsupported', message: 'Album search is not supported by this source yet.' };
    const hits = DEMO_TRACKS.filter((t) => `${t.title} ${t.artist}`.toLowerCase().includes(q));
    let items: SearchItemDto[];
    if (filter === 'artists') {
      items = [...new Set(hits.map((t) => t.artist))].map((name) => ({ type: 'artist', key: `demo-artist:${name}`, name, thumbnail: null }));
    } else if (filter === 'playlists') {
      items = hits.length ? [{ type: 'playlist', key: 'demo-playlist:1', title: `${query.trim()} mix`, uploader: 'Pyxee demo', thumbnail: null }] : [];
    } else {
      items = hits.map((track) => ({ type: 'track', track }));
    }
    return { items };
  }
  async onState(cb: (s: PlayerStateDto) => void) {
    this.#listeners.add(cb);
    return () => void this.#listeners.delete(cb);
  }

  async dispatch(c: CommandDto) {
    this.#apply(c);
    this.#publish();
  }

  /** Advances simulated time. Exposed so tests can drive it deterministically. */
  tick(ms: number) {
    const s = this.#s;
    if (s.transport.state === 'loading') {
      this.#loadMs -= ms;
      if (this.#loadMs <= 0) {
        this.#newListen();
        s.audible = s.transport.id; s.loading = null;
        s.transport = { state: 'playing', id: s.audible };
      }
    } else if (s.transport.state === 'playing') {
      s.positionMs += ms;
      const entry = this.#entry(s.transport.id);
      const dur = entry?.track.durationMs ?? null;
      this.#listenedMs += ms;
      if (entry && !this.#counted && this.#listenedMs >= Math.min(30_000, dur ? dur / 2 : 30_000)) {
        this.#counted = true;
        this.recordPlay(entry.track);
      }
      if (dur != null && s.positionMs >= dur) this.#ended(s.transport.id);
    } else {
      return;
    }
    this.#publish();
  }

  #entry(id: number | null) { return this.#s.queue.find((e) => e.id === id); }
  #index(id: number | null) { return this.#s.queue.findIndex((e) => e.id === id); }

  #publish() {
    const s = this.#s;
    const i = this.#index(s.current);
    s.upcoming = i < 0 ? s.queue.map((e) => e.id) : s.queue.slice(i + 1).map((e) => e.id);
    s.canUndo = this.#undo.length > 0;
    const snapshot = clone(s);
    for (const cb of this.#listeners) cb(snapshot);
  }

  #load(id: number, startMs = 0) {
    const s = this.#s;
    s.current = id; s.loading = id; s.audible = null; s.positionMs = startMs;
    s.transport = { state: 'loading', id };
    this.#loadMs = 500;
  }

  #stop() {
    const s = this.#s;
    s.transport = { state: 'idle' }; s.audible = null; s.loading = null; s.positionMs = 0;
  }

  #ended(id: number) {
    const s = this.#s;
    if (s.repeat === 'one') { s.positionMs = 0; this.#newListen(); return; }
    const i = this.#index(id);
    const next = s.queue[i + 1] ?? (s.repeat === 'all' ? s.queue[0] : undefined);
    if (!next) return this.#stop();
    // Gapless: the next track becomes audible immediately.
    this.#newListen();
    s.current = next.id; s.audible = next.id; s.positionMs = 0;
    s.transport = { state: 'playing', id: next.id };
  }

  #apply(c: CommandDto) {
    const s = this.#s;
    switch (c.type) {
      case 'playTracks': {
        s.queue = c.tracks.map((track) => ({ id: this.#next++, track, origin: 'user' as const }));
        this.#undo = [];
        const first = s.queue[Math.min(c.start, s.queue.length - 1)];
        if (first) this.#load(first.id);
        break;
      }
      case 'playEntry': if (this.#entry(c.id)) this.#load(c.id); break;
      case 'toggle':
        if (s.transport.state === 'playing' || s.transport.state === 'stalled') this.#apply({ type: 'pause' });
        else this.#apply({ type: 'resume' });
        break;
      case 'pause':
        if (s.transport.state === 'playing') s.transport = { state: 'paused', id: s.transport.id, loaded: true };
        break;
      case 'resume':
        if (s.transport.state === 'paused') {
          if (s.transport.loaded) s.transport = { state: 'playing', id: s.transport.id };
          else this.#load(s.transport.id, s.positionMs);
        } else if (s.transport.state === 'idle') {
          const id = s.current ?? s.queue[0]?.id;
          if (id != null) this.#load(id);
        }
        break;
      case 'next': {
        const i = this.#index(s.current);
        const next = s.queue[i + 1] ?? (s.repeat === 'all' ? s.queue[0] : undefined);
        if (next) this.#load(next.id); else this.#stop();
        break;
      }
      case 'previous': {
        const prev = s.queue[this.#index(s.current) - 1];
        if (s.positionMs > 3000 || !prev) s.positionMs = 0; else this.#load(prev.id);
        break;
      }
      case 'seek': s.positionMs = c.ms; break;
      case 'setRepeat': s.repeat = c.repeat; break;
      case 'setShuffle': s.shuffle = c.on; break;
      case 'setAutoplay': s.autoplay = c.on; break;
      case 'addToQueue': s.queue.push({ id: this.#next++, track: c.track, origin: 'user' }); break;
      case 'playNext': s.queue.splice(this.#index(s.current) + 1, 0, { id: this.#next++, track: c.track, origin: 'user' }); break;
      case 'remove': {
        const i = this.#index(c.id);
        if (i < 0) break;
        const [entry] = s.queue.splice(i, 1);
        if (entry) this.#undo.push({ entry, index: i });
        if (s.current === c.id) {
          const next = s.queue[i];
          if (next) this.#load(next.id); else { this.#stop(); s.current = null; }
        }
        break;
      }
      case 'move': {
        const i = this.#index(c.id);
        const [entry] = i < 0 ? [] : s.queue.splice(i, 1);
        if (entry) s.queue.splice(Math.min(c.to, s.queue.length), 0, entry);
        break;
      }
      case 'moveUpcoming': {
        const base = this.#index(s.current) + 1;
        const i = this.#index(c.id);
        if (i < base || c.id === s.current) break;
        const [entry] = s.queue.splice(i, 1);
        if (entry) s.queue.splice(Math.min(base + c.to, s.queue.length), 0, entry);
        break;
      }
      case 'clear':
        if (s.queue.length) this.#undo.push({ cleared: s.queue, current: s.current });
        s.queue = []; s.current = null; this.#stop();
        break;
      case 'undo': {
        const u = this.#undo.pop();
        if (u && 'cleared' in u) { s.queue = u.cleared; s.current = u.current; }
        else if (u) s.queue.splice(Math.min(u.index, s.queue.length), 0, u.entry);
        break;
      }
      case 'retry': break;
    }
  }
}
