import { afterEach, describe, expect, it, vi } from 'vitest';
import { createEvent, fireEvent, render } from '@testing-library/svelte';
import TrackRow from './TrackRow.svelte';
import EditableRow from './EditableRow.svelte';
import { createRawSnippet } from 'svelte';
import { nav } from '$lib/nav.svelte';
import { DEMO_TRACKS } from '$lib/mock';

afterEach(() => {
  nav.menuTrack = null;
});

describe('right-clicking a song', () => {
  it('opens the same menu as its "more" button, and not the browser\'s', async () => {
    const track = DEMO_TRACKS[0]!;
    const { container } = render(TrackRow, { track, onplay: () => {} });
    const row = container.querySelector('li')!;
    const event = createEvent.contextMenu(row);
    await fireEvent(row, event);
    expect(event.defaultPrevented).toBe(true);
    expect(nav.menuTrack).toEqual(track);
  });

  it('does the same on a row of the queue, a playlist or Up next', async () => {
    const track = DEMO_TRACKS[1]!;
    const actions = createRawSnippet(() => ({ render: () => '<span></span>' }));
    const { container } = render(EditableRow, { track, onplay: () => {}, actions });
    const row = container.querySelector('li')!;
    const event = createEvent.contextMenu(row.querySelector('button')!);
    await fireEvent(row.querySelector('button')!, event);
    expect(event.defaultPrevented).toBe(true);
    expect(nav.menuTrack).toEqual(track);
  });
});

describe('a row reveals itself as it scrolls into view', () => {
  // jsdom has no IntersectionObserver: a stand-in the test can fire by hand.
  class Watcher {
    static last: Watcher | null = null;
    callback: IntersectionObserverCallback;
    targets = new Set<Element>();
    constructor(cb: IntersectionObserverCallback) {
      this.callback = cb;
      Watcher.last = this;
    }
    observe(el: Element) {
      this.targets.add(el);
    }
    unobserve(el: Element) {
      this.targets.delete(el);
    }
    disconnect() {
      this.targets.clear();
    }
  }
  const entry = (target: Element) => ({
    target,
    isIntersecting: true,
    boundingClientRect: { top: 80, left: 0 },
    intersectionRect: { top: 80, left: 0 },
  });
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    Watcher.last = null;
  });

  it('waits invisible, then plays the row and its cover once, and is left alone afterwards', () => {
    vi.stubGlobal('IntersectionObserver', Watcher);
    const animate = vi.spyOn(Element.prototype, 'animate');
    const { container } = render(TrackRow, { track: DEMO_TRACKS[0]!, onplay: () => {} });
    const row = container.querySelector('li')!;
    expect(row.hasAttribute('data-reveal')).toBe(true);
    expect(animate).not.toHaveBeenCalled();

    Watcher.last!.callback([entry(row)] as unknown as IntersectionObserverEntry[], Watcher.last as unknown as IntersectionObserver);
    expect(row.hasAttribute('data-reveal')).toBe(false);
    const animated = animate.mock.contexts.filter((el): el is Element => el instanceof Element);
    expect(animated).toContain(row);
    expect(animated.some((el) => el.hasAttribute('data-cover'))).toBe(true);
    expect(Watcher.last!.targets.size).toBe(0);
  });

  it('is simply there where nothing can watch it (an old webview)', () => {
    const { container } = render(TrackRow, { track: DEMO_TRACKS[0]!, onplay: () => {} });
    expect(container.querySelector('li')!.hasAttribute('data-reveal')).toBe(false);
  });
});
