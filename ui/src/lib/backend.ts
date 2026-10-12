import type { CommandDto } from './bindings/CommandDto';
import type { AddResultDto } from './bindings/AddResultDto';
import type { FilterInfoDto } from './bindings/FilterInfoDto';
import type { HistoryItemDto } from './bindings/HistoryItemDto';
import type { LogInfoDto } from './bindings/LogInfoDto';
import type { NoticeDto } from './bindings/NoticeDto';
import type { HomeDto } from './bindings/HomeDto';
import type { LibrarySnapshotDto } from './bindings/LibrarySnapshotDto';
import type { PlaylistDto } from './bindings/PlaylistDto';
import type { PlaylistSnapshotDto } from './bindings/PlaylistSnapshotDto';
import type { RemovedEntryDto } from './bindings/RemovedEntryDto';
import type { SearchErrorDto } from './bindings/SearchErrorDto';
import type { SearchFilterDto } from './bindings/SearchFilterDto';
import type { SearchResponseDto } from './bindings/SearchResponseDto';
import type { PlayerStateDto } from './bindings/PlayerStateDto';
import type { TrackDto } from './bindings/TrackDto';
import type { WindowInfoDto } from './bindings/WindowInfoDto';

/** Library calls. Failures reject with a plain-sentence string the UI can show as is. */
export interface LibraryBackend {
  getLibrary(): Promise<LibrarySnapshotDto>;
  onLibrary(cb: (s: LibrarySnapshotDto) => void): Promise<() => void>;
  setLiked(track: TrackDto, liked: boolean): Promise<void>;
  likedGet(): Promise<TrackDto[]>;
  playlistCreate(name: string, tracks: TrackDto[]): Promise<number>;
  playlistRename(id: number, name: string): Promise<void>;
  playlistDelete(id: number): Promise<PlaylistSnapshotDto>;
  playlistRestore(snapshot: PlaylistSnapshotDto): Promise<void>;
  playlistAdd(id: number, tracks: TrackDto[]): Promise<AddResultDto>;
  playlistGet(id: number): Promise<PlaylistDto>;
  playlistRemoveEntry(entryId: number): Promise<RemovedEntryDto>;
  playlistRestoreEntry(removed: RemovedEntryDto): Promise<number>;
  playlistMoveEntry(entryId: number, to: number): Promise<void>;
  historyGet(limit: number): Promise<HistoryItemDto[]>;
  historyClear(): Promise<HistoryItemDto[]>;
  historyRestore(items: HistoryItemDto[]): Promise<void>;
  homeGet(): Promise<HomeDto>;
}

/** Everything the UI needs from Rust. Implemented by Tauri at runtime and by a mock in the browser and in tests. */
export interface Backend extends LibraryBackend {
  getState(): Promise<PlayerStateDto>;
  dispatch(command: CommandDto): Promise<void>;
  /** Subscribes to state pushes; resolves to an unsubscribe function. */
  onState(cb: (state: PlayerStateDto) => void): Promise<() => void>;
  windowInfo(): Promise<WindowInfoDto>;
  /** Output volume, 0-100. */
  setVolume(percent: number): Promise<void>;
  /** Where the log file is, for the Diagnostics section. */
  logInfo(): Promise<LogInfoDto>;
  openLogFolder(): Promise<void>;
  /** Messages for the listener that are not player state (a song was skipped, a stream was cut off). */
  onNotice(cb: (message: string) => void): Promise<() => void>;
  /** Rejects with a `SearchErrorDto` (offline, rate limited, tool missing, ...). */
  search(query: string, filter: SearchFilterDto): Promise<SearchResponseDto>;
  /** Live suggestions; never rejects (no suggestions beats an error while typing). */
  suggest(prefix: string): Promise<string[]>;
  searchFilters(): Promise<FilterInfoDto[]>;
  /** Demo tracks for the simulated engine; removed once real search lands (milestone 4). */
  devCatalog(): Promise<TrackDto[]>;
}

export function toSearchError(e: unknown): SearchErrorDto {
  if (typeof e === 'object' && e !== null && 'kind' in e && 'message' in e) return e as SearchErrorDto;
  return { kind: 'other', message: e instanceof Error ? e.message : String(e) };
}

export const STATE_EVENT = 'player-state';
export const LIBRARY_EVENT = 'library-changed';
export const NOTICE_EVENT = 'notice';

/** Error text from a rejected call: Rust sends plain strings; anything else gets a generic sentence. */
export function messageOf(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return 'Something went wrong. Please try again.';
}

export async function tauriBackend(): Promise<Backend> {
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');
  return {
    getState: () => invoke<PlayerStateDto>('get_state'),
    dispatch: (command) => invoke<void>('dispatch', { command }),
    onState: async (cb) => listen<PlayerStateDto>(STATE_EVENT, (e) => cb(e.payload)),
    windowInfo: () => invoke<WindowInfoDto>('window_info'),
    setVolume: (percent) => invoke<void>('set_volume', { percent: Math.round(percent) }),
    logInfo: () => invoke<LogInfoDto>('get_log_info'),
    openLogFolder: () => invoke<void>('open_log_folder'),
    onNotice: async (cb) => listen<NoticeDto>(NOTICE_EVENT, (e) => cb(e.payload.message)),
    search: (query, filter) => invoke<SearchResponseDto>('search', { query, filter }),
    suggest: (prefix) => invoke<string[]>('suggest', { prefix }).catch(() => []),
    searchFilters: () => invoke<FilterInfoDto[]>('search_filters'),
    getLibrary: () => invoke<LibrarySnapshotDto>('get_library'),
    onLibrary: async (cb) => listen<LibrarySnapshotDto>(LIBRARY_EVENT, (e) => cb(e.payload)),
    setLiked: (track, liked) => invoke<void>('library_set_liked', { track, liked }),
    likedGet: () => invoke<TrackDto[]>('liked_get'),
    playlistCreate: (name, tracks) => invoke<number>('playlist_create', { name, tracks }),
    playlistRename: (id, name) => invoke<void>('playlist_rename', { id, name }),
    playlistDelete: (id) => invoke<PlaylistSnapshotDto>('playlist_delete', { id }),
    playlistRestore: (snapshot) => invoke<void>('playlist_restore', { snapshot }),
    playlistAdd: (id, tracks) => invoke<AddResultDto>('playlist_add', { id, tracks }),
    playlistGet: (id) => invoke<PlaylistDto>('playlist_get', { id }),
    playlistRemoveEntry: (entryId) => invoke<RemovedEntryDto>('playlist_remove_entry', { entryId }),
    playlistRestoreEntry: (removed) => invoke<number>('playlist_restore_entry', { removed }),
    playlistMoveEntry: (entryId, to) => invoke<void>('playlist_move_entry', { entryId, to }),
    historyGet: (limit) => invoke<HistoryItemDto[]>('history_get', { limit }),
    historyClear: () => invoke<HistoryItemDto[]>('history_clear'),
    historyRestore: (items) => invoke<void>('history_restore', { items }),
    homeGet: () => invoke<HomeDto>('home_get'),
    devCatalog: () => invoke<TrackDto[]>('dev_catalog'),
  };
}

export async function createBackend(): Promise<Backend> {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) return tauriBackend();
  const { MockBackend } = await import('./mock');
  const mock = new MockBackend();
  mock.start();
  return mock;
}
