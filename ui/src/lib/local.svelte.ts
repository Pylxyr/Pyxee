import type { LibraryBackend } from './backend';
import type { LocalLibraryDto } from './bindings/LocalLibraryDto';

const EMPTY: LocalLibraryDto = { revision: 0, folders: [], trackCount: 0, scanning: false, unreachable: [] };

/** The songs on this PC at a glance: the chosen folders and where the scan of them stands. The songs are asked for by the page. */
export class LocalStore {
  info = $state<LocalLibraryDto>(EMPTY);
  ready = $state(false);
  #unlisten: (() => void) | null = null;

  async init(backend: LibraryBackend) {
    this.#unlisten?.();
    this.info = EMPTY;
    this.#unlisten = await backend.onLocal((s) => this.#apply(s));
    this.#apply(await backend.localGet());
    this.ready = true;
  }

  /** Keeps the newest: a push and an answer can arrive in either order, and an older one must not replace a newer one. */
  #apply(s: LocalLibraryDto) {
    if (s.revision >= this.info.revision) this.info = s;
  }
}

export const local = new LocalStore();
