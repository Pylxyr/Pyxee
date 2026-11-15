import { isNotFound, messageOf } from './backend';

/**
 * One piece of data a page loads from the backend, and what the page needs to show it honestly: whether it has arrived
 * yet (`loaded`), whether a read is under way (`loading`), the last error (`error`, kept apart from the data), and
 * whether the thing is really gone (`missing`, only for a not-found answer).
 *
 * A read that finishes after a newer one was started is dropped, so an older answer never replaces a newer one. A
 * failed read keeps the data that is already shown; `reset()` forgets everything (another playlist was opened).
 */
export class Loaded<T> {
  data = $state.raw<T | null>(null);
  loaded = $state(false);
  loading = $state(false);
  error = $state('');
  missing = $state(false);
  #latest = 0;

  async load(read: () => Promise<T>): Promise<void> {
    const mine = ++this.#latest;
    this.loading = true;
    try {
      const value = await read();
      if (mine !== this.#latest) return;
      this.data = value;
      this.loaded = true;
      this.error = '';
      this.missing = false;
    } catch (e) {
      if (mine !== this.#latest) return;
      if (isNotFound(e)) {
        this.data = null;
        this.missing = true;
        this.loaded = true;
        this.error = '';
      } else {
        this.error = messageOf(e);
      }
    } finally {
      if (mine === this.#latest) this.loading = false;
    }
  }

  /** Forgets the data and drops any read that is still on its way. */
  reset(): void {
    this.#latest++;
    this.data = null;
    this.loaded = false;
    this.loading = false;
    this.error = '';
    this.missing = false;
  }
}
