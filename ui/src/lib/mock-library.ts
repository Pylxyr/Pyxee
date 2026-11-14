import type { ExploreShelfDto } from './bindings/ExploreShelfDto';
import type { GenreDto } from './bindings/GenreDto';
import type { AddResultDto } from './bindings/AddResultDto';
import type { HistoryItemDto } from './bindings/HistoryItemDto';
import type { HomeDto } from './bindings/HomeDto';
import type { LibrarySnapshotDto } from './bindings/LibrarySnapshotDto';
import type { PlaylistDto } from './bindings/PlaylistDto';
import type { PlaylistSnapshotDto } from './bindings/PlaylistSnapshotDto';
import type { RemovedEntryDto } from './bindings/RemovedEntryDto';
import type { TrackDto } from './bindings/TrackDto';
import type { LibraryBackend } from './backend';

const NAME_ERROR = 'Playlist names must be 1 to 100 characters.';
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T;

interface Pl { id: number; name: string; createdAt: number; updatedAt: number; entries: { entryId: number; track: TrackDto }[] }

/**
 * In-memory stand-in for the Rust library (browser dev and tests). It follows the same rules as
 * `pyxee-library`: names are trimmed and limited, adding skips songs already present, delete / remove /
 * clear return what is needed to undo them. Rust is the source of truth; keep this in step with it.
 */
export class MockLibrary implements LibraryBackend {
  #liked: TrackDto[] = [];
  #playlists: Pl[] = [];
  #history: HistoryItemDto[] = [];
  #nextPlaylist = 1;
  #nextEntry = 1;
  #revision = 1;
  #listeners = new Set<(s: LibrarySnapshotDto) => void>();
  #now = () => Date.now();

  #changed() {
    this.#revision++;
    const s = this.#snapshot();
    for (const cb of this.#listeners) cb(clone(s));
  }

  #snapshot(): LibrarySnapshotDto {
    return {
      revision: this.#revision,
      likedKeys: this.#liked.map((t) => t.key),
      playlists: [...this.#playlists]
        .sort((a, b) => b.updatedAt - a.updatedAt || b.id - a.id)
        .map((p) => ({ id: p.id, name: p.name, trackCount: p.entries.length, updatedAt: p.updatedAt, thumbnail: p.entries[0]?.track.thumbnail ?? null })),
      historyCount: this.#history.length,
      error: null,
    };
  }

  #pl(id: number): Pl {
    const p = this.#playlists.find((x) => x.id === id);
    if (!p) throw 'That playlist no longer exists.';
    return p;
  }

  static #name(name: string): string {
    const n = name.trim();
    if (!n || [...n].length > 100) throw NAME_ERROR;
    return n;
  }

  /** Called by the mock player when a song has really been listened to. */
  recordPlay(track: TrackDto) {
    this.#history.unshift({ track: clone(track), playedAt: this.#now() });
    this.#changed();
  }

  async getLibrary() { return clone(this.#snapshot()); }
  async onLibrary(cb: (s: LibrarySnapshotDto) => void) {
    this.#listeners.add(cb);
    return () => void this.#listeners.delete(cb);
  }

  async setLiked(track: TrackDto, liked: boolean) {
    const has = this.#liked.some((t) => t.key === track.key);
    if (liked && !has) this.#liked.unshift(clone(track));
    else if (!liked && has) this.#liked = this.#liked.filter((t) => t.key !== track.key);
    else return;
    this.#changed();
  }
  async likedGet() { return clone(this.#liked); }

  async playlistCreate(name: string, tracks: TrackDto[]) {
    const n = MockLibrary.#name(name);
    const now = this.#now();
    const id = this.#nextPlaylist++;
    this.#playlists.push({ id, name: n, createdAt: now, updatedAt: now, entries: tracks.map((t) => ({ entryId: this.#nextEntry++, track: clone(t) })) });
    this.#changed();
    return id;
  }
  async playlistRename(id: number, name: string) {
    const p = this.#pl(id);
    p.name = MockLibrary.#name(name);
    p.updatedAt = this.#now();
    this.#changed();
  }
  async playlistDelete(id: number): Promise<PlaylistSnapshotDto> {
    const p = this.#pl(id);
    this.#playlists = this.#playlists.filter((x) => x.id !== id);
    this.#changed();
    return { id: p.id, name: p.name, createdAt: p.createdAt, updatedAt: p.updatedAt, tracks: p.entries.map((e) => e.track) };
  }
  async playlistRestore(s: PlaylistSnapshotDto) {
    this.#playlists.push({ id: s.id, name: s.name, createdAt: s.createdAt, updatedAt: s.updatedAt, entries: s.tracks.map((t) => ({ entryId: this.#nextEntry++, track: clone(t) })) });
    this.#changed();
  }
  async playlistAdd(id: number, tracks: TrackDto[]): Promise<AddResultDto> {
    const p = this.#pl(id);
    const present = new Set(p.entries.map((e) => e.track.key));
    let added = 0, skipped = 0;
    for (const t of tracks) {
      if (present.has(t.key)) { skipped++; continue; }
      present.add(t.key);
      p.entries.push({ entryId: this.#nextEntry++, track: clone(t) });
      added++;
    }
    p.updatedAt = this.#now();
    this.#changed();
    return { added, skipped };
  }
  async playlistGet(id: number): Promise<PlaylistDto> { return clone(this.#pl(id)); }
  async playlistRemoveEntry(entryId: number): Promise<RemovedEntryDto> {
    const p = this.#playlists.find((x) => x.entries.some((e) => e.entryId === entryId));
    if (!p) throw 'That playlist entry no longer exists.';
    const position = p.entries.findIndex((e) => e.entryId === entryId);
    const [e] = p.entries.splice(position, 1);
    p.updatedAt = this.#now();
    this.#changed();
    return { playlistId: p.id, position, track: e!.track };
  }
  async playlistRestoreEntry(r: RemovedEntryDto) {
    const p = this.#pl(r.playlistId);
    const entryId = this.#nextEntry++;
    p.entries.splice(Math.min(r.position, p.entries.length), 0, { entryId, track: clone(r.track) });
    p.updatedAt = this.#now();
    this.#changed();
    return entryId;
  }
  async playlistMoveEntry(entryId: number, to: number) {
    const p = this.#playlists.find((x) => x.entries.some((e) => e.entryId === entryId));
    if (!p) throw 'That playlist entry no longer exists.';
    const from = p.entries.findIndex((e) => e.entryId === entryId);
    const [e] = p.entries.splice(from, 1);
    p.entries.splice(Math.min(to, p.entries.length), 0, e!);
    p.updatedAt = this.#now();
    this.#changed();
  }

  async historyGet(limit: number) { return clone(this.#history.slice(0, limit)); }
  async historyClear() {
    const all = this.#history;
    this.#history = [];
    this.#changed();
    return clone(all);
  }
  async historyRestore(items: HistoryItemDto[]) {
    this.#history = [...clone(items), ...this.#history].sort((a, b) => b.playedAt - a.playedAt);
    this.#changed();
  }

  /** Related songs: the demo backend overrides this; the bare library mock knows no catalogue. */
  async relatedGet(_key: string, _count: number): Promise<TrackDto[]> { return []; }
  /** YouTube Music's Explore shelves: the demo backend overrides this. */
  /** Genres, a genre's shelves and a playlist's songs: the demo backend overrides these. */
  async genresGet(): Promise<GenreDto[]> { return []; }
  async genreGet(_params: string): Promise<ExploreShelfDto[]> { return []; }
  async playlistTracksGet(_id: string): Promise<TrackDto[]> { return []; }
  async homeGet(): Promise<HomeDto> {
    const counts = new Map<string, { track: TrackDto; plays: number }>();
    for (const h of this.#history) {
      const e = counts.get(h.track.key) ?? { track: h.track, plays: 0 };
      e.plays++;
      counts.set(h.track.key, e);
    }
    for (const t of this.#liked) if (!counts.has(t.key)) counts.set(t.key, { track: t, plays: 0 });
    const liked = new Set(this.#liked.map((t) => t.key));
    const quickPicks = [...counts.values()]
      .map((e) => ({ ...e, score: e.plays * 2 + (liked.has(e.track.key) ? 6 : 0) }))
      .sort((a, b) => b.score - a.score)
      .slice(0, 12)
      .map((e) => e.track);
    const seen = new Set<string>();
    const recent = this.#history.filter((h) => !seen.has(h.track.key) && seen.add(h.track.key)).slice(0, 12).map((h) => h.track);
    return clone({ quickPicks, recent, likedCount: this.#liked.length });
  }
}
