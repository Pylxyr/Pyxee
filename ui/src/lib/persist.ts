// The page's own settings (theme, custom themes, Look, interface size, volume, crossfade, seek style, search filter) are
// saved in the web view's `localStorage`, which belongs to the web view's profile and not to Pyxee's data folder: a
// profile reset, "Delete app data" when uninstalling, a moved portable copy or a different build (development or
// installed) starts with none of it. So the app keeps a copy in `ui-prefs.json` in the data folder (the equalizer, the
// tray and the other shell settings are already in `shell.json`), and a start puts back what the web view is missing.
//
// - `restoreSettings` runs before anything reads the store (the stores read it when their module loads): it fills in
//   the entries the web view does not have, and never replaces one it has.
// - `persist.touch()` is called after each write; a moment later the whole set is sent to the app (one write for a burst
//   of changes, none if nothing differs from what the file already holds).
// Outside the app window (a browser, the tests) there is no file and all of this does nothing.

/** Every entry the page keeps starts with this (the app refuses anything else). */
export const PREFIX = 'pyxee.';

export type Items = Record<string, string>;
export type Reader = Pick<Storage, 'length' | 'key' | 'getItem' | 'setItem'>;
export interface PrefsFile {
  get(): Promise<Items>;
  set(items: Items): Promise<void>;
}

const tauriFile: PrefsFile = {
  get: async () => (await import('@tauri-apps/api/core')).invoke<Items>('ui_prefs_get'),
  set: async (items) => (await import('@tauri-apps/api/core')).invoke<void>('ui_prefs_set', { items }),
};

const inApp = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** How long a start waits for the app's answer before going on with what the web view has. */
const ASK_MS = 1500;
/** The quiet time after a change before it is sent (settings change in bursts: a slider, a theme editor). */
const SEND_AFTER_MS = 600;

/** The page's entries in `storage`, in a fixed order. */
export function snapshot(storage: Reader): Items {
  const found: Items = {};
  try {
    for (let i = 0; i < storage.length; i++) {
      const key = storage.key(i);
      if (key?.startsWith(PREFIX)) {
        const value = storage.getItem(key);
        if (value !== null) found[key] = value;
      }
    }
  } catch {
    /* a store that cannot be read has nothing to copy */
  }
  return Object.fromEntries(Object.entries(found).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
}

/** What the file holds that `storage` does not: put back, entry by entry. Resolves to the file's content, or null if it could not be read. */
export async function restore(storage: Reader, file: PrefsFile, waitMs = ASK_MS): Promise<{ saved: Items | null; restored: string[] }> {
  let saved: Items | null = null;
  try {
    saved = await Promise.race([file.get(), new Promise<never>((_, no) => setTimeout(() => no(new Error('timed out')), waitMs))]);
  } catch {
    return { saved: null, restored: [] };
  }
  const restored: string[] = [];
  for (const [key, value] of Object.entries(saved)) {
    if (!key.startsWith(PREFIX) || typeof value !== 'string') continue;
    try {
      if (storage.getItem(key) === null) {
        storage.setItem(key, value);
        restored.push(key);
      }
    } catch {
      /* a full or blocked store: the entry stays in the file */
    }
  }
  return { saved, restored };
}

/** Sends the page's entries to the app when they differ from what it holds. */
export class Mirror {
  #last: string | null;
  #timer: ReturnType<typeof setTimeout> | undefined;
  readonly #storage: Reader;
  readonly #file: PrefsFile;
  readonly #delay: number;

  /** `known`: what the file held at start (null: unknown, so the first change is always sent). */
  constructor(storage: Reader, file: PrefsFile, known: Items | null, delayMs = SEND_AFTER_MS) {
    this.#storage = storage;
    this.#file = file;
    this.#delay = delayMs;
    this.#last = known === null ? null : JSON.stringify(known);
  }

  /** A setting was written: send the set a moment later (a burst of writes is one send). */
  touch(): void {
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => void this.flush(), this.#delay);
  }

  /** Sends now (also when the window is hidden or closing). A failed send is tried again with the next change. */
  async flush(): Promise<void> {
    clearTimeout(this.#timer);
    const items = snapshot(this.#storage);
    const text = JSON.stringify(items);
    if (text === this.#last) return;
    try {
      await this.#file.set(items);
      this.#last = text;
    } catch {
      /* kept in the web view's store; the next change sends it again */
    }
  }
}

let mirror: Mirror | null = null;

export const persist = {
  /** Call after writing a setting to `localStorage`. */
  touch(): void {
    mirror?.touch();
  },
};

/**
 * Called once at start, before the app's modules are loaded. Puts back what the web view lost and, in the main window,
 * starts copying changes to the data folder (the small windows only read). `file` is for tests.
 */
export async function restoreSettings(opts: { mirror: boolean; storage?: Reader; file?: PrefsFile } = { mirror: true }): Promise<void> {
  const file = opts.file ?? (inApp() ? tauriFile : null);
  const storage = opts.storage ?? (typeof localStorage !== 'undefined' ? localStorage : null);
  if (!file || !storage) return;
  const { saved } = await restore(storage, file);
  if (!opts.mirror) return;
  mirror = new Mirror(storage, file, saved);
  // The first start after an update has settings the file has never seen (or the file could not be read): copy them now.
  void mirror.flush();
  if (typeof document !== 'undefined') {
    document.addEventListener('visibilitychange', () => {
      if (document.hidden) void mirror?.flush();
    });
    window.addEventListener('pagehide', () => void mirror?.flush());
  }
}

/** For tests: forget the mirror. */
export function resetPersist(): void {
  mirror = null;
}
