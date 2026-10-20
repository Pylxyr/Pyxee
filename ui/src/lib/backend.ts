import type { CommandDto } from './bindings/CommandDto';
import type { AddResultDto } from './bindings/AddResultDto';
import type { FilterInfoDto } from './bindings/FilterInfoDto';
import type { HistoryItemDto } from './bindings/HistoryItemDto';
import type { LogInfoDto } from './bindings/LogInfoDto';
import type { EnvSettingsDto } from './bindings/EnvSettingsDto';
import type { EnvVarDto } from './bindings/EnvVarDto';
import type { AppUpdateDto } from './bindings/AppUpdateDto';
import type { YtdlpUpdateDto } from './bindings/YtdlpUpdateDto';
import type { UpdateProgressDto } from './bindings/UpdateProgressDto';
import type { LyricsDto } from './bindings/LyricsDto';
import type { ShellSettingsDto } from './bindings/ShellSettingsDto';
import type { SleepSettingDto } from './bindings/SleepSettingDto';
import type { SleepStateDto } from './bindings/SleepStateDto';
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
  /** Songs similar to `key` (for Explore). Rejects with a `SearchErrorDto` like `search` does. */
  relatedGet(key: string, count: number): Promise<TrackDto[]>;
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
  /** Makes the native window frame and backdrop follow the chosen theme; `null` follows the system again. */
  setWindowTheme(theme: 'dark' | 'light' | null): Promise<void>;
  /** Where the log file is, for the Diagnostics section. */
  logInfo(): Promise<LogInfoDto>;
  openLogFolder(): Promise<void>;
  /** The variables kept in the `.env` file (Settings > Advanced). */
  envGet(): Promise<EnvSettingsDto>;
  /** Saves them to `.env`; they are used from the next start. Rejects with a readable message for a value that cannot be used. */
  envSet(vars: EnvVarDto[]): Promise<EnvSettingsDto>;
  /** Looks for a newer Pyxee release on GitHub. */
  updateCheckApp(): Promise<AppUpdateDto>;
  /** Downloads the installer, checks its SHA-256, starts it and quits Pyxee. Rejects with a readable message. */
  updateInstallApp(): Promise<void>;
  /** Compares the yt-dlp in use with the published one. */
  updateCheckYtdlp(): Promise<YtdlpUpdateDto>;
  /** Downloads, verifies and swaps in the newest yt-dlp; resolves to its version. The old one is kept if anything fails. */
  updateInstallYtdlp(): Promise<string>;
  /** Where a Pyxee or yt-dlp install is (downloading, verifying, testing, starting); only while one runs. */
  onUpdateProgress(cb: (p: UpdateProgressDto) => void): Promise<() => void>;
  /** Messages for the listener that are not player state (a song was skipped, a stream was cut off). */
  onNotice(cb: (message: string) => void): Promise<() => void>;
  /** Rejects with a `SearchErrorDto` (offline, rate limited, tool missing, ...). */
  search(query: string, filter: SearchFilterDto): Promise<SearchResponseDto>;
  /** Live suggestions; never rejects (no suggestions beats an error while typing). */
  suggest(prefix: string): Promise<string[]>;
  searchFilters(): Promise<FilterInfoDto[]>;
  /** Demo tracks for the simulated engine; removed once real search lands (milestone 4). */
  devCatalog(): Promise<TrackDto[]>;

  /** Lyrics from LRCLIB; never rejects (a failed lookup is `{ kind: 'error' }`). */
  lyricsGet(track: TrackDto): Promise<LyricsDto>;
  sleepSet(setting: SleepSettingDto): Promise<void>;
  sleepCancel(): Promise<void>;
  /** "+15 minutes" on a running time timer (starts one if none runs). */
  sleepExtend(minutes: number): Promise<void>;
  sleepGet(): Promise<SleepStateDto>;
  onSleep(cb: (s: SleepStateDto) => void): Promise<() => void>;
  shellGet(): Promise<ShellSettingsDto>;
  /** Saves the tray / start-with-Windows / toast settings; resolves to what is now in effect. */
  shellSet(settings: ShellSettingsDto): Promise<ShellSettingsDto>;
  /** Tray flyout and mini player. */
  flyoutHide(): Promise<void>;
  flyoutOpenMain(): Promise<void>;
  miniToggle(): Promise<void>;
  mainShow(): Promise<void>;
}

export function toSearchError(e: unknown): SearchErrorDto {
  if (typeof e === 'object' && e !== null && 'kind' in e && 'message' in e) return e as SearchErrorDto;
  return { kind: 'other', message: e instanceof Error ? e.message : String(e) };
}

export const STATE_EVENT = 'player-state';
export const LIBRARY_EVENT = 'library-changed';
export const NOTICE_EVENT = 'notice';
export const UPDATE_PROGRESS_EVENT = 'update-progress';
export const SLEEP_EVENT = 'sleep-state';

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
    setWindowTheme: (theme) => invoke<void>('set_window_theme', { theme }).catch(() => {}),
    logInfo: () => invoke<LogInfoDto>('get_log_info'),
    openLogFolder: () => invoke<void>('open_log_folder'),
    envGet: () => invoke<EnvSettingsDto>('env_get'),
    envSet: (vars) => invoke<EnvSettingsDto>('env_set', { vars }),
    updateCheckApp: () => invoke<AppUpdateDto>('update_check_app'),
    updateInstallApp: () => invoke<void>('update_install_app'),
    updateCheckYtdlp: () => invoke<YtdlpUpdateDto>('update_check_ytdlp'),
    updateInstallYtdlp: () => invoke<string>('update_install_ytdlp'),
    onUpdateProgress: async (cb) => listen<UpdateProgressDto>(UPDATE_PROGRESS_EVENT, (e) => cb(e.payload)),
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
    relatedGet: (key, count) => invoke<TrackDto[]>('related_get', { key, count }),
    devCatalog: () => invoke<TrackDto[]>('dev_catalog'),
    lyricsGet: (t) =>
      invoke<LyricsDto>('lyrics_get', { key: t.key, title: t.title, artist: t.artist, durationMs: t.durationMs }).catch(
        (e): LyricsDto => ({ kind: 'error', message: messageOf(e) }),
      ),
    sleepSet: (setting) => invoke<void>('sleep_set', { setting }),
    sleepCancel: () => invoke<void>('sleep_cancel'),
    sleepExtend: (minutes) => invoke<void>('sleep_extend', { minutes }),
    sleepGet: () => invoke<SleepStateDto>('sleep_get'),
    onSleep: async (cb) => listen<SleepStateDto>(SLEEP_EVENT, (e) => cb(e.payload)),
    shellGet: () => invoke<ShellSettingsDto>('shell_get'),
    shellSet: (settings) => invoke<ShellSettingsDto>('shell_set', { settings }),
    flyoutHide: () => invoke<void>('flyout_hide'),
    flyoutOpenMain: () => invoke<void>('flyout_open_main'),
    miniToggle: () => invoke<void>('mini_toggle'),
    mainShow: () => invoke<void>('main_show'),
  };
}

export async function createBackend(): Promise<Backend> {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) return tauriBackend();
  const { MockBackend } = await import('./mock');
  const mock = new MockBackend();
  mock.start();
  return mock;
}
