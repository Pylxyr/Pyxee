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
};

/** Diagnostics methods for test backends that do not care about them. */
export const noDiagnostics = {
  logInfo: async () => ({ dir: '', file: '', error: null }),
  openLogFolder: async () => {},
  onNotice: async () => () => {},
};
