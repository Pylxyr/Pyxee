/**
 * The window's size and pixel density as far as pictures care. WebView2 can leave `loading="lazy"` covers blank (and
 * pictures sized for the old density) after a fullscreen switch or a move to another monitor, so every size or density
 * change bumps `epoch`; a cover that has not loaded then starts over (see Thumb.svelte).
 */
class Viewport {
  epoch = $state(0);
  dpr = $state(typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1);
  #watching = false;
  #timer: ReturnType<typeof setTimeout> | null = null;

  /** Starts listening (once). */
  watch() {
    if (this.#watching || typeof window === 'undefined') return;
    this.#watching = true;
    const changed = () => {
      if (this.#timer) clearTimeout(this.#timer);
      // Fullscreen fires a burst of resizes; one reaction after the layout has settled is enough.
      this.#timer = setTimeout(() => this.bump(), 200);
    };
    window.addEventListener('resize', changed);
    document.addEventListener('fullscreenchange', changed);
  }

  bump() {
    this.dpr = typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1;
    this.epoch++;
  }
}

export const viewport = new Viewport();
