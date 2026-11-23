import type { LibraryBackend } from './backend';

/** Library methods for test backends that do not care about the library: harmless defaults. */
export const noLibrary: LibraryBackend = {
  getLibrary: async () => ({ revision: 1, likedKeys: [], playlists: [], historyCount: 0, error: null }),
  onLibrary: async () => () => {},
  setLiked: async () => {},
  likedGet: async () => [],
  playlistCreate: async () => 1,
  playlistRename: async () => {},
  playlistDelete: async () => ({ id: 1, name: '', createdAt: 0, updatedAt: 0, tracks: [] }),
  playlistRestore: async () => {},
  playlistAdd: async () => ({ added: 0, skipped: 0 }),
  playlistGet: async () => ({ id: 1, name: '', createdAt: 0, updatedAt: 0, entries: [] }),
  playlistRemoveEntry: async () => ({ playlistId: 1, position: 0, track: { key: '', title: '', artist: '', durationMs: null, thumbnail: null } }),
  playlistRestoreEntry: async () => 1,
  playlistMoveEntry: async () => {},
  historyGet: async () => [],
  historyClear: async () => [],
  historyRemove: async () => [],
  historyRestore: async () => {},
  homeGet: async () => ({ quickPicks: [], recent: [], likedCount: 0 }),
  relatedGet: async () => [],
  genresGet: async () => [],
  genreGet: async () => [],
  playlistTracksGet: async () => [],
  localGet: async () => ({ revision: 1, folders: [], trackCount: 0, scanning: false, unreachable: [] }),
  onLocal: async () => () => {},
  localTracks: async () => [],
  localAddFolder: async () => ({ revision: 1, folders: [], trackCount: 0, scanning: false, unreachable: [] }),
  localRemoveFolder: async () => ({ revision: 1, folders: [], trackCount: 0, scanning: false, unreachable: [] }),
  localRescan: async () => ({ revision: 1, folders: [], trackCount: 0, scanning: false, unreachable: [] }),
  audioCacheGet: async () => ({ songs: 0, usedMb: 0, limitMb: 1024 }),
  audioCacheSetLimit: async (limitMb: number) => ({ songs: 0, usedMb: 0, limitMb }),
  audioCacheClear: async () => ({ songs: 0, usedMb: 0, limitMb: 1024 }),
  localMissing: async () => [],
};

/** Diagnostics methods for test backends that do not care about them. */
export const noDiagnostics = {
  setWindowTheme: async () => {},
  setWindowTint: async () => {},
  logInfo: async () => ({ dir: '', file: '', enabled: false, error: null }),
  openLogFolder: async () => {},
  openReleasePage: async () => {},
  onNotice: async () => () => {},
  onUpdateProgress: async () => () => {},
  updateCheckApp: async () => ({ current: '0.1.0', latest: '0.1.0', available: false, notesUrl: '', notes: '', installable: false, portable: false }),
  updateInstallApp: async () => {},
  updateCheckYtdlp: async () => ({ available: false, currentVersion: null }),
  updateInstallYtdlp: async () => '',
  perfEnabled: async () => false,
  seekMode: async () => 'step',
  platform: async () => 'windows',
  perfLog: async () => {},
};

/** Lyrics, sleep timer, tray and shell-settings methods for test backends that do not care about them. */
export const noShell = {
  coverBytes: async (): Promise<ArrayBuffer> => {
    throw new Error('no cover');
  },
  lyricsGet: async () => ({ kind: 'notFound' as const }),
  sleepSet: async () => {},
  sleepCancel: async () => {},
  sleepExtend: async () => {},
  sleepGet: async () => ({ status: null }),
  onSleep: async () => () => {},
  eqGet: async () => ({ enabled: false, preamp: 0, gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }),
  eqSet: async (eq: { enabled: boolean; preamp: number; gains: number[] }) => eq,
  shellGet: async () => ({ closeToTray: false, startWithWindows: false, toastOnChange: false, trayControls: false, flyoutOnClick: true, discordPresence: false, lastfmScrobbling: false, checkPrereleases: false }),
  shellSet: async (s: { closeToTray: boolean; startWithWindows: boolean; toastOnChange: boolean; trayControls: boolean; flyoutOnClick: boolean; discordPresence: boolean; lastfmScrobbling: boolean; checkPrereleases: boolean }) => s,
  flyoutHide: async () => {},
  flyoutOpenMain: async () => {},
  miniToggle: async () => {},
  mainShow: async () => {},
};
