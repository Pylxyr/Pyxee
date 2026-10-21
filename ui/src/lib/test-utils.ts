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
  historyRestore: async () => {},
  homeGet: async () => ({ quickPicks: [], recent: [], likedCount: 0 }),
  relatedGet: async () => [],
};

/** Diagnostics methods for test backends that do not care about them. */
export const noDiagnostics = {
  setWindowTheme: async () => {},
  logInfo: async () => ({ dir: '', file: '', error: null }),
  openLogFolder: async () => {},
  onNotice: async () => () => {},
  onUpdateProgress: async () => () => {},
  updateCheckApp: async () => ({ current: '0.1.0', latest: '0.1.0', available: false, notesUrl: '', notes: '', installable: false, portable: false }),
  updateInstallApp: async () => {},
  updateCheckYtdlp: async () => ({ available: false, currentVersion: null }),
  updateInstallYtdlp: async () => '',
  envGet: async () => ({ file: null, vars: [] }),
  envSet: async () => ({ file: null, vars: [] }),
};

/** Lyrics, sleep timer, tray and shell-settings methods for test backends that do not care about them. */
export const noShell = {
  lyricsGet: async () => ({ kind: 'notFound' as const }),
  sleepSet: async () => {},
  sleepCancel: async () => {},
  sleepExtend: async () => {},
  sleepGet: async () => ({ status: null }),
  onSleep: async () => () => {},
  shellGet: async () => ({ closeToTray: false, startWithWindows: false, toastOnChange: false, trayControls: false, flyoutOnClick: true, discordPresence: true }),
  shellSet: async (s: { closeToTray: boolean; startWithWindows: boolean; toastOnChange: boolean; trayControls: boolean; flyoutOnClick: boolean; discordPresence: boolean }) => s,
  flyoutHide: async () => {},
  flyoutOpenMain: async () => {},
  miniToggle: async () => {},
  mainShow: async () => {},
};
