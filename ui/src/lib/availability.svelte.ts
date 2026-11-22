import { library } from './library.svelte';

/** How long an answer is trusted before the file is looked at again (a drive can come back, a file can be restored). */
const FRESH_MS = 30_000;
/** Keys asked about in one call. */
const BATCH = 500;
/** Rows ask one by one as they appear; asking waits this long so a whole list goes in one call. */
const GATHER_MS = 60;

/**
 * Which songs on this PC have lost their file (moved, renamed, deleted, or on a drive that is not connected), so lists
 * can say so before the song is played and skipped. Rows call `want` for their song; the answers come from one call per
 * list. Only `file:` keys are asked about; a song from YouTube is never missing in this sense.
 */
export class AvailabilityStore {
  /** The keys found to be missing at the last look. A new set replaces it, so readers run only when it changed. */
  missing = $state.raw<ReadonlySet<string>>(new Set());
  /** Rows read this in the effect that calls `want`, so they ask again after `reset`. */
  epoch = $state(0);
  #looked = new Map<string, number>();
  #wanted = new Set<string>();
  #timer: ReturnType<typeof setTimeout> | null = null;

  isMissing(key: string): boolean {
    return this.missing.has(key);
  }

  /** A row shows `key`: look at its file soon, unless that was done a moment ago. */
  want(key: string): void {
    if (!key.startsWith('file:')) return;
    const at = this.#looked.get(key);
    if (at !== undefined && Date.now() - at < FRESH_MS) return;
    this.#wanted.add(key);
    this.#timer ??= setTimeout(() => void this.#flush(), GATHER_MS);
  }

  /** Forgets what was looked at, so the next rows that show a song look again (after a scan, or a folder came back). */
  reset(): void {
    this.#looked.clear();
    this.epoch++;
  }

  async #flush(): Promise<void> {
    this.#timer = null;
    const keys = [...this.#wanted];
    this.#wanted.clear();
    const backend = library.backend;
    if (!backend || keys.length === 0) return;
    for (let i = 0; i < keys.length; i += BATCH) {
      const batch = keys.slice(i, i + BATCH);
      try {
        const gone = new Set(await backend.localMissing(batch));
        const now = Date.now();
        for (const k of batch) this.#looked.set(k, now);
        const next = new Set(this.missing);
        for (const k of batch) {
          if (gone.has(k)) next.add(k);
          else next.delete(k);
        }
        if (next.size !== this.missing.size || [...next].some((k) => !this.missing.has(k))) this.missing = next;
      } catch {
        /* the check is a courtesy: a failed look marks nothing */
      }
    }
  }
}

export const availability = new AvailabilityStore();
