/**
 * Whether the PC says it is online, and a counter that goes up every time it comes back online. Covers that could
 * not be fetched wait for that signal instead of polling while there is no connection (see Thumb.svelte).
 */
class Network {
  online = $state(typeof navigator === 'undefined' ? true : navigator.onLine !== false);
  /** Bumped on every offline -> online change. */
  epoch = $state(0);
  #watching = false;

  /** Starts listening (once). */
  watch() {
    if (this.#watching || typeof window === 'undefined') return;
    this.#watching = true;
    window.addEventListener('online', () => {
      this.online = true;
      this.epoch++;
    });
    window.addEventListener('offline', () => {
      this.online = false;
    });
  }
}

export const network = new Network();
