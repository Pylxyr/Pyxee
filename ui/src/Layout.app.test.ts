import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { DEMO_TRACKS, MockBackend } from '$lib/mock';
import { library } from '$lib/library.svelte';
import { nav } from '$lib/nav.svelte';
import { player, EMPTY_STATE } from '$lib/player.svelte';
import { prefs } from '$lib/prefs.svelte';
import { search } from '$lib/search.svelte';
import { snackbar } from '$lib/snackbar.svelte';

let mock: MockBackend;
vi.mock('$lib/backend', async (orig) => ({
  ...(await orig<typeof import('$lib/backend')>()),
  createBackend: async () => mock,
}));

import App from './App.svelte';

function setWide(wide: boolean) {
  window.matchMedia = ((q: string) => ({
    matches: wide && q.includes('min-width'), media: q,
    addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, dispatchEvent: () => false, onchange: null,
  })) as typeof window.matchMedia;
}

beforeEach(() => {
  mock = new MockBackend();
  mock.searchLatencyMs = 0;
  player.state = EMPTY_STATE;
  player.ready = false;
  player.catalog = null;
  library.ready = false;
  library.snapshot = { revision: 0, likedKeys: [], playlists: [], historyCount: 0, error: null };
  Object.assign(nav, { route: 'home', nowPlayingOpen: false, menuTrack: null, pickTrack: null, namePrompt: null, libraryTab: 'liked', playlistId: null, themeSheetOpen: false, tab: 'upnext' });
  search.query = ''; search.status = 'idle'; search.items = []; search.error = null; search.suggestions = [];
  for (const s of [...snackbar.items]) snackbar.dismiss(s.id);
  prefs.setVolume(80);
  setWide(false);
});

async function openApp() {
  render(App);
  await waitFor(() => expect(player.ready && library.ready).toBe(true));
}

/** True if `el` or any ancestor up to (not including) `stop` is a scroll container. */
function scrollsWithin(el: Element, stop: Element): boolean {
  for (let e: Element | null = el; e && e !== stop; e = e.parentElement) {
    if (e.classList.contains('overflow-y-auto')) return true;
  }
  return false;
}

describe('Now Playing layout: only the list scrolls', () => {
  async function openNowPlaying() {
    await openApp();
    await fireEvent.click(await screen.findByRole('button', { name: /^Neon Harbor/ }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Open Now Playing' }));
    return screen.findByTestId('np-root');
  }

  it('the overlay itself never scrolls', async () => {
    const root = await openNowPlaying();
    expect(root.classList.contains('overflow-hidden')).toBe(true);
    expect(root.classList.contains('overflow-y-auto')).toBe(false);
  });

  it('the header, tabs, art and controls are outside every scroll container', async () => {
    const root = await openNowPlaying();
    for (const id of ['np-header', 'np-tabs', 'np-art']) {
      expect(scrollsWithin(within(root).getByTestId(id), root), `${id} must not scroll`).toBe(false);
    }
    // The controls column may scroll only as a last resort on the wide layout (lg:), never at the base layout.
    const controls = within(root).getByTestId('np-controls');
    expect(controls.classList.contains('overflow-y-auto')).toBe(false);
    expect(within(controls).getByRole('button', { name: 'Previous' })).toBeTruthy();
    expect(scrollsWithin(within(controls).getByRole('button', { name: 'Next' }), root)).toBe(false);
  });

  it('the list is the one scroll container, and it holds the queue', async () => {
    const root = await openNowPlaying();
    const scroll = within(root).getByTestId('np-scroll');
    expect(scroll.classList.contains('overflow-y-auto')).toBe(true);
    expect(scroll.classList.contains('min-h-0')).toBe(true);
    expect(within(scroll).getByText('Paper Planes')).toBeTruthy();
    expect(scroll.contains(within(root).getByTestId('np-tabs'))).toBe(false);
    expect(scroll.contains(within(root).getByTestId('np-controls'))).toBe(false);
    expect(root.querySelectorAll('.overflow-y-auto').length).toBe(1);
  });

  it('the cover and title column is not centred with justify-center, which cuts off its top when the window is short', async () => {
    await openApp();
    await player.playTracks([DEMO_TRACKS[0]!]);
    await waitFor(() => expect(player.shown).not.toBeNull());
    nav.openNowPlaying();
    const controls = within(await screen.findByTestId('np-root')).getByTestId('np-controls');
    expect(controls.classList.contains('justify-center')).toBe(false);
    expect(controls.className).toContain('[&>:first-child]:mt-auto');
    expect(controls.className).toContain('[&>:last-child]:mb-auto');
  });

  it('the art scales with its box instead of being cropped to a fixed size', async () => {
    await openApp();
    await player.playTracks([{ ...DEMO_TRACKS[0]!, thumbnail: 'https://example.invalid/a.jpg' }]);
    await waitFor(() => expect(player.shown).not.toBeNull());
    nav.openNowPlaying();
    const root = await screen.findByTestId('np-root');
    const art = within(within(root).getByTestId('np-art')).getByRole('presentation', { hidden: true }) as HTMLImageElement;
    expect(art.classList.contains('h-full') && art.classList.contains('w-full')).toBe(true);
    expect(art.style.width).toBe('');
  });
});

describe('pages keep their header in place while the body scrolls', () => {
  function expectPinnedHeader() {
    const header = screen.getByTestId('page-header');
    const body = screen.getByTestId('page-scroll');
    expect(body.classList.contains('overflow-y-auto')).toBe(true);
    expect(body.contains(header)).toBe(false);
    expect(header.classList.contains('shrink-0')).toBe(true);
    expect(header.parentElement!.classList.contains('h-full')).toBe(true);
  }

  it('the app shell does not scroll as a whole; pages manage their own scrolling', async () => {
    await openApp();
    const main = document.querySelector('main')!;
    expect(main.classList.contains('overflow-hidden')).toBe(true);
    expect(main.classList.contains('overflow-y-auto')).toBe(false);
  });

  it('Queue: title and buttons stay, the list scrolls', async () => {
    await openApp();
    await fireEvent.click(await screen.findByRole('button', { name: /^Neon Harbor/ }));
    await waitFor(() => expect(player.state.queue.length).toBeGreaterThan(3));
    await fireEvent.click(screen.getByRole('button', { name: /Queue/ }));
    await screen.findByRole('list', { name: 'Queue' });
    expectPinnedHeader();
    expect(within(screen.getByTestId('page-header')).getByRole('button', { name: 'Clear queue' })).toBeTruthy();
    expect(within(screen.getByTestId('page-scroll')).getByRole('list', { name: 'Queue' })).toBeTruthy();
  });

  it('Library: the title and tabs stay, the lists scroll', async () => {
    await mock.setLiked(DEMO_TRACKS[0]!, true);
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: /Library/ }));
    await screen.findByRole('list', { name: 'Liked songs' });
    expectPinnedHeader();
    expect(within(screen.getByTestId('page-header')).getByRole('tab', { name: 'Playlists' })).toBeTruthy();
  });

  it('Playlist: name and actions stay, the songs scroll', async () => {
    const id = await mock.playlistCreate('Mix', DEMO_TRACKS.slice(0, 5));
    await openApp();
    nav.openPlaylist(id);
    await screen.findByRole('list', { name: 'Songs in Mix' });
    expectPinnedHeader();
    expect(within(screen.getByTestId('page-header')).getByRole('button', { name: 'Play' })).toBeTruthy();
    expect(within(screen.getByTestId('page-scroll')).getByRole('list', { name: 'Songs in Mix' })).toBeTruthy();
  });

  it('the search box does not spell-check', async () => {
    await openApp();
    expect(screen.getByRole('combobox', { name: 'Search' }).getAttribute('spellcheck')).toBe('false');
  });

  it('Search: the heading and filters stay, the results scroll', async () => {
    await openApp();
    const box = screen.getByRole('combobox', { name: 'Search' });
    await fireEvent.input(box, { target: { value: 'a' } });
    await fireEvent.submit(box.closest('form')!);
    await screen.findByRole('button', { name: /^Neon Harbor/ });
    expectPinnedHeader();
    expect(within(screen.getByTestId('page-header')).getByRole('button', { name: 'Songs' })).toBeTruthy();
  });
});

describe('volume reaches the audio engine', () => {
  it('the saved volume is sent to the engine as soon as the app is ready', async () => {
    prefs.setVolume(35);
    await openApp();
    await waitFor(() => expect(mock.volume).toBe(35));
  });

  it('moving the slider changes the engine volume immediately, and keeps the setting', async () => {
    setWide(true);
    await openApp();
    const slider = await screen.findByRole('slider', { name: 'Volume' }).catch(async () => {
      await fireEvent.click(await screen.findByRole('button', { name: /^Neon Harbor/ })); // the bar appears with a song
      return screen.findByRole('slider', { name: 'Volume' });
    });
    await fireEvent.input(slider, { target: { value: '30' } });
    await waitFor(() => expect(mock.volume).toBe(30));
    expect(prefs.volume).toBe(30);
    await fireEvent.input(slider, { target: { value: '0' } });
    await waitFor(() => expect(mock.volume).toBe(0));
    await fireEvent.input(slider, { target: { value: '100' } });
    await waitFor(() => expect(mock.volume).toBe(100));
  });

  it('out-of-range values are kept within 0-100', async () => {
    await openApp();
    prefs.setVolume(250);
    await waitFor(() => expect(mock.volume).toBe(100));
    prefs.setVolume(-5);
    await waitFor(() => expect(mock.volume).toBe(0));
  });
});
