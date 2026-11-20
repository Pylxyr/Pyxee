import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/svelte';

// jsdom has no Web Animations API; Svelte transitions call element.animate().
if (!Element.prototype.animate) {
  Element.prototype.animate = function () {
    // Finish at once (next tick): Svelte waits for `onfinish` before removing an element after its outro. As in a
    // browser, a cancelled animation never reports that it finished (Svelte cancels the ones of a block that is torn down).
    let timer: ReturnType<typeof setTimeout> | undefined;
    const a: Record<string, unknown> = {
      cancel() {
        clearTimeout(timer);
      },
      finish() {},
      finished: Promise.resolve(),
      _cb: null,
      // A real Animation is an EventTarget: `burst()` listens for `finish` this way instead of through `onfinish`.
      // Same rule as above: it reports at once (next tick), and never after a cancel.
      addEventListener(type: string, cb: () => void) {
        if (type === 'finish') timer = setTimeout(() => cb(), 0);
      },
      removeEventListener() {},
    };
    Object.defineProperty(a, 'onfinish', {
      set(cb: (() => void) | null) {
        a._cb = cb;
        timer = setTimeout(() => cb?.(), 0);
      },
      get: () => a._cb,
    });
    return a as unknown as Animation;
  };
}

// jsdom has no matchMedia; default to a narrow, light-system window. Tests can override it.
if (!window.matchMedia) {
  window.matchMedia = (query: string) =>
    ({ matches: false, media: query, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, dispatchEvent: () => false, onchange: null }) as MediaQueryList;
}

afterEach(() => cleanup());
