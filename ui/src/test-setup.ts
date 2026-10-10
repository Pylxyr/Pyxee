import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/svelte';

// jsdom has no Web Animations API; Svelte transitions call element.animate().
if (!Element.prototype.animate) {
  Element.prototype.animate = function () {
    // Finish immediately: Svelte waits for `onfinish` before removing an element after its outro.
    const a: Record<string, unknown> = { cancel() {}, finish() {}, finished: Promise.resolve(), _cb: null };
    Object.defineProperty(a, 'onfinish', {
      set(cb: (() => void) | null) {
        a._cb = cb;
        setTimeout(() => cb?.(), 0);
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
