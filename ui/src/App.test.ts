import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { DEMO_TRACKS, MockBackend } from '$lib/mock';
import { nav } from '$lib/nav.svelte';
import { player, EMPTY_STATE } from '$lib/player.svelte';
import { snackbar } from '$lib/snackbar.svelte';
import { search } from '$lib/search.svelte';
import { library } from '$lib/library.svelte';
import { explore } from '$lib/explore.svelte';
import { theme } from '$lib/theme.svelte';
import { BUILTIN_THEMES } from '$lib/themes';

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
  search.query = '';
  search.filter = 'all';
  search.status = 'idle';
  search.items = [];
  search.error = null;
  search.suggestions = [];
  player.state = EMPTY_STATE;
  player.ready = false;
  player.catalog = null;
  library.ready = false;
  nav.route = 'home';
  nav.nowPlayingOpen = false;
  nav.menuTrack = null;
  nav.themeSheetOpen = false;
  nav.themeEditorOpen = false;
  nav.tab = 'upnext';
  theme.preview = null;
  theme.settings = { followSystem: true, themeId: 'dark', lightId: 'light', darkId: 'dark', dynamicColor: false, custom: [] };
  theme.systemDark = true;
  for (const s of [...snackbar.items]) snackbar.dismiss(s.id);
  setWide(false);
});

async function openApp() {
  render(App);
  // The stores are singletons: wait until THIS app instance is connected to THIS backend.
  await waitFor(() => expect(player.ready && library.ready).toBe(true));
  await screen.findByText(DEMO_TRACKS[0]!.title);
}

describe('App shell', () => {
  it('shows skeletons first, then the demo tracks, and no player bar before anything plays', async () => {
    render(App);
    expect(screen.getByLabelText('Loading')).toBeTruthy();
    await screen.findByText(DEMO_TRACKS[0]!.title);
    expect(screen.queryByTestId('player-bar')).toBeNull();
  });

  it('uses the bottom bar when narrow and the top tabs when wide, with the same destinations', async () => {
    await openApp();
    const narrow = screen.getAllByRole('navigation', { name: 'Main' });
    expect(narrow).toHaveLength(1);
    expect(narrow[0]!.className).toContain('border-t');
    expect(narrow[0]!.textContent).toMatch(/Home.*Explore.*Library/);
  });

  it('wide layout puts the wordmark and the four destinations in tabs along the top', async () => {
    setWide(true);
    await openApp();
    const tabs = screen.getByRole('navigation', { name: 'Main' });
    expect(tabs.className).not.toContain('border-t');
    expect(tabs.closest('header, div')?.className).toContain('border-b');
    expect(tabs.textContent).toMatch(/Pyxee.*Home.*Explore.*Library.*Queue/);
    expect(screen.queryByRole('navigation', { name: 'Main' })?.querySelector('[aria-current="page"]')?.textContent).toContain('Home');
  });

  it('navigates between Home, Explore and Library', async () => {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: /Library/ }));
    expect(screen.getByRole('heading', { name: 'Library' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: /Explore/ }));
    expect(screen.getByRole('heading', { name: 'Explore' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: /Home/ }));
    expect(screen.getByRole('heading', { name: 'On rotation' })).toBeTruthy();
  });

  it('Explore offers moods that open a search, and shelves built from what you like', async () => {
    explore.status = 'idle';
    mock.likedGet = async () => [DEMO_TRACKS[0]!];
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: /Explore/ }));
    expect(await screen.findByRole('heading', { name: `Because you liked ${DEMO_TRACKS[0]!.title}` })).toBeTruthy();
    expect(await screen.findByRole('list', { name: `Because you liked ${DEMO_TRACKS[0]!.title}` })).toBeTruthy();
    expect(screen.queryByText(/Explore is not built yet/)).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Search Focus' }));
    await waitFor(() => expect(nav.route).toBe('search'));
    expect(search.query).toBe('focus instrumental');
  });

  it('searching opens the results page with the query', async () => {
    await openApp();
    const box = screen.getByRole('combobox', { name: 'Search' });
    await fireEvent.input(box, { target: { value: '  lowtide ' } });
    await fireEvent.submit(box.closest('form')!);
    expect(screen.getByRole('heading', { name: /Results for/ }).textContent).toContain('lowtide');
  });
});

describe('playing from the UI', () => {
  it('clicking a track shows it as loading, then playing only once the backend says it is audible', async () => {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: /^Slow Orbit/ }));
    const bar = await screen.findByTestId('player-bar');
    expect(bar.textContent).toContain('Slow Orbit');
    expect(screen.getByRole('button', { name: /Loading/ })).toBeTruthy();
    mock.tick(600);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Pause' })).toBeTruthy());
  });

  it('play/pause and next work from the bar and update what is shown', async () => {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: /^Neon Harbor/ }));
    await screen.findByTestId('player-bar');
    mock.tick(600);
    await fireEvent.click(await screen.findByRole('button', { name: 'Pause' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Play' })).toBeTruthy());
    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    await waitFor(() => expect(screen.getByTestId('player-bar').textContent).toContain('Paper Planes'));
  });

  it('keyboard: space toggles and n skips, but typing in search does not', async () => {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: /^Neon Harbor/ }));
    mock.tick(600);
    await fireEvent.keyDown(document.body, { key: ' ' });
    await waitFor(() => expect(player.state.transport.state).toBe('paused'));
    await fireEvent.keyDown(document.body, { key: 'n' });
    await waitFor(() => expect(player.shown?.track.title).toBe('Paper Planes'));
    const box = screen.getByRole('combobox', { name: 'Search' });
    await fireEvent.keyDown(box, { key: 'n' });
    expect(player.shown?.track.title).toBe('Paper Planes');
  });

  it('shows a failure with a retry button in the player bar', async () => {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: /^Neon Harbor/ }));
    const id = player.state.current!;
    player.state = { ...player.state, transport: { state: 'failed', id, failure: { kind: 'offline' } }, loading: null };
    expect((await screen.findByRole('alert')).textContent).toMatch(/offline/i);
    await fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
  });
});

describe('row menu (in-page sheet) and snackbars', () => {
  it('Play next / Add to queue go to the backend and confirm with a snackbar', async () => {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: 'More options for Glass Rooms' }));
    const dialog = await screen.findByRole('dialog', { name: 'Glass Rooms' });
    expect(dialog).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Add to queue' }));
    await waitFor(() => expect(player.state.queue.map((e) => e.track.title)).toEqual(['Glass Rooms']));
    expect(snackbar.items.at(-1)?.message).toBe('Added to queue');
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Glass Rooms' })).toBeNull());
  });
});

describe('Now Playing', () => {
  async function playing() {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: /^Neon Harbor/ }));
    mock.tick(600);
    await screen.findByRole('button', { name: 'Pause' });
  }

  it('opens from the bar, lists up next, and closes with Escape', async () => {
    await playing();
    await fireEvent.click(screen.getByRole('button', { name: 'Open Now Playing' }));
    const np = await screen.findByRole('region', { name: 'Now Playing' });
    expect(np.textContent).toContain('Up next');
    expect(np.textContent).toContain('Paper Planes');
    await fireEvent.keyDown(document.body, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('region', { name: 'Now Playing' })).toBeNull());
  });

  it('removing a queued track offers Undo, and Undo puts it back', async () => {
    await playing();
    await fireEvent.click(screen.getByRole('button', { name: 'Open Now Playing' }));
    const before = player.state.queue.length;
    await fireEvent.click(await screen.findByRole('button', { name: 'Remove Paper Planes from queue' }));
    await waitFor(() => expect(player.state.queue).toHaveLength(before - 1));
    const snack = snackbar.items.at(-1)!;
    expect(snack.message).toContain('Paper Planes');
    expect(snack.action?.label).toBe('Undo');
    snackbar.act(snack.id);
    await waitFor(() => expect(player.state.queue).toHaveLength(before));
  });

  it('Up next can be reordered with its buttons: Play next puts a song right after the current one', async () => {
    await playing();
    await fireEvent.click(screen.getByRole('button', { name: 'Open Now Playing' }));
    const names = () => player.upNext.map((e) => e.track.title);
    const before = names();
    expect(before.length).toBeGreaterThan(2);
    const last = before[before.length - 1];
    await fireEvent.click(await screen.findByRole('button', { name: `Play ${last} next` }));
    await waitFor(() => expect(names()[0]).toBe(last));
    expect(names().slice(1)).toEqual(before.slice(0, -1));
    // The first row has nothing above it to move to.
    expect(screen.getByRole('button', { name: `Move ${last} up` }).hasAttribute('disabled')).toBe(true);
    await fireEvent.click(screen.getByRole('button', { name: `Move ${last} down` }));
    await waitFor(() => expect(names()[1]).toBe(last));
  });

  it('tabs switch panels: Lyrics shows the lyrics, Up next the queue', async () => {
    await playing();
    await fireEvent.click(screen.getByRole('button', { name: 'Open Now Playing' }));
    await fireEvent.click(await screen.findByRole('tab', { name: 'Lyrics' }));
    expect(within(screen.getByRole('tabpanel')).getByRole('region', { name: 'Lyrics' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('tab', { name: 'Up next' }));
    expect(screen.getByRole('tabpanel').textContent).toContain('Paper Planes');
  });

  it('shuffle and repeat buttons reflect and change state', async () => {
    await playing();
    await fireEvent.click(screen.getByRole('button', { name: 'Open Now Playing' }));
    const shuffle = await screen.findByRole('button', { name: 'Shuffle' });
    expect(shuffle.getAttribute('aria-pressed')).toBe('false');
    await fireEvent.click(shuffle);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Shuffle' }).getAttribute('aria-pressed')).toBe('true'));
    await fireEvent.click(screen.getByRole('button', { name: 'Repeat: off' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Repeat: all' })).toBeTruthy());
  });
});

describe('appearance', () => {
  async function openAppearance() {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    return screen.findByRole('group', { name: 'Theme' });
  }

  it('switching to Frost (light) and Ink (dark) sets the theme on the document', async () => {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Frost' }));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('light'));
    await fireEvent.click(screen.getByRole('button', { name: 'Ink' }));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('dark'));
  });

  it('offers Ink, Frost and the other themes (and no YT Music look-alike), and applies the one picked', async () => {
    const group = await openAppearance();
    for (const name of ['Ink', 'Frost', 'OLED black', 'Deep blue', 'Graphite and gold']) expect(within(group).getByRole('button', { name })).toBeTruthy();
    expect(within(group).queryByRole('button', { name: 'YT Music' })).toBeNull();
    await fireEvent.click(within(group).getByRole('button', { name: 'OLED black' }));
    await waitFor(() => expect(document.documentElement.dataset.themeId).toBe('oled'));
    expect(document.documentElement.style.getPropertyValue('--t-accent')).toBe('#ff5c6a');
    expect(within(group).getByRole('button', { name: 'OLED black' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('tells the native window which theme to follow (a fixed one, or the system again)', async () => {
    const group = await openAppearance();
    await fireEvent.click(within(group).getByRole('button', { name: 'Deep blue' }));
    await waitFor(() => expect(mock.windowTheme).toBe('dark'));
    await fireEvent.click(within(group).getByRole('button', { name: 'Sky' }));
    await waitFor(() => expect(mock.windowTheme).toBe('light'));
    await fireEvent.click(screen.getByRole('checkbox', { name: /Match system/ }));
    await waitFor(() => expect(mock.windowTheme).toBeNull());
  });

  it('while matching the system, one theme is used for light and another for dark', async () => {
    await openAppearance();
    await fireEvent.change(screen.getByRole('combobox', { name: 'Dark theme' }), { target: { value: 'ember' } });
    await waitFor(() => expect(theme.settings.darkId).toBe('ember'));
  });

  it('customizing: edits show live, Cancel throws them away, Save keeps them as a new theme', async () => {
    await openAppearance();
    await fireEvent.click(screen.getByRole('button', { name: 'Customize…' }));
    const editor = await screen.findByRole('dialog', { name: 'Customize theme' });
    const accent = within(editor).getByRole('textbox', { name: 'Accent hex' });
    await fireEvent.input(accent, { target: { value: '#00ff88' } });
    await waitFor(() => expect(document.documentElement.style.getPropertyValue('--t-accent')).toBe('#00ff88'));
    await fireEvent.click(within(editor).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(document.documentElement.style.getPropertyValue('--t-accent')).not.toBe('#00ff88'));
    expect(theme.settings.custom).toHaveLength(0);

    await fireEvent.click(screen.getByRole('button', { name: 'Customize…' }));
    const again = await screen.findByRole('dialog', { name: 'Customize theme' });
    await fireEvent.input(within(again).getByRole('textbox', { name: 'Theme name' }), { target: { value: 'Neon' } });
    await fireEvent.input(within(again).getByRole('textbox', { name: 'Accent hex' }), { target: { value: '#00ff88' } });
    await fireEvent.click(within(again).getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(theme.settings.custom.map((t) => t.name)).toEqual(['Neon']));
    expect(theme.active.name).toBe('Neon');
    expect(document.documentElement.style.getPropertyValue('--t-accent')).toBe('#00ff88');
    expect(within(screen.getByRole('group', { name: 'Theme' })).getByRole('button', { name: 'Neon' })).toBeTruthy();
  });

  it('warns when the colours chosen are hard to read', async () => {
    await openAppearance();
    await fireEvent.click(screen.getByRole('button', { name: 'Customize…' }));
    const editor = await screen.findByRole('dialog', { name: 'Customize theme' });
    await fireEvent.input(within(editor).getByRole('textbox', { name: 'Text hex' }), { target: { value: '#101010' } });
    expect((await within(editor).findByRole('status', { name: 'Readability' })).textContent).toContain('Text on background');
  });

  it('a theme can be exported and imported again', async () => {
    const copied: string[] = [];
    Object.assign(navigator, { clipboard: { writeText: async (t: string) => void copied.push(t) } });
    await openAppearance();
    await fireEvent.click(screen.getByRole('button', { name: 'Customize…' }));
    const editor = await screen.findByRole('dialog', { name: 'Customize theme' });
    await fireEvent.input(within(editor).getByRole('textbox', { name: 'Theme name' }), { target: { value: 'Shared' } });
    await fireEvent.click(within(editor).getByRole('button', { name: 'Export' }));
    await waitFor(() => expect(copied).toHaveLength(1));
    await fireEvent.input(within(editor).getByRole('textbox', { name: 'Theme name' }), { target: { value: 'Changed' } });
    await fireEvent.input(within(editor).getByRole('textbox', { name: 'Theme text' }), { target: { value: copied[0]! } });
    await fireEvent.click(within(editor).getByRole('button', { name: 'Import' }));
    await waitFor(() => expect((within(editor).getByRole('textbox', { name: 'Theme name' }) as HTMLInputElement).value).toBe('Shared'));
    await fireEvent.input(within(editor).getByRole('textbox', { name: 'Theme text' }), { target: { value: 'garbage' } });
    await fireEvent.click(within(editor).getByRole('button', { name: 'Import' }));
    expect((await within(editor).findByRole('alert')).textContent).toContain("isn't theme text");
  });

  it('a saved theme can be deleted', async () => {
    theme.saveCustom({ id: '', name: 'Temp', scheme: 'dark', colors: { ...BUILTIN_THEMES[0]!.colors } });
    theme.select(theme.settings.custom[0]!.id);
    await openAppearance();
    await fireEvent.click(screen.getByRole('button', { name: 'Customize…' }));
    const editor = await screen.findByRole('dialog', { name: 'Customize theme' });
    await fireEvent.click(within(editor).getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(theme.settings.custom).toHaveLength(0));
    expect(theme.active.id).toBe('dark');
  });
});

describe('search', () => {
  async function typeAndSubmit(text: string) {
    const box = screen.getByRole('combobox', { name: 'Search' });
    await fireEvent.input(box, { target: { value: text } });
    await fireEvent.submit(box.closest('form')!);
  }

  it('shows skeletons, then results with artist and duration, and clicking plays the song', async () => {
    mock.searchLatencyMs = 40;
    await openApp();
    await typeAndSubmit('lowtide');
    expect(screen.getByRole('heading', { name: /Results for/ }).textContent).toContain('lowtide');
    expect(screen.getByLabelText('Loading')).toBeTruthy();
    const row = await screen.findByRole('button', { name: /^Neon Harbor/ });
    expect(row.textContent).toContain('Lowtide');
    expect(row.textContent).toContain('0:38');
    await fireEvent.click(row);
    await waitFor(() => expect(player.shown?.track.title).toBe('Neon Harbor'));
    expect(player.state.queue).toHaveLength(1);
  });

  it('suggestions appear while typing and picking one searches for it', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      await openApp();
      const box = screen.getByRole('combobox', { name: 'Search' });
      await fireEvent.input(box, { target: { value: 'lowt' } });
      await vi.advanceTimersByTimeAsync(200);
      const options = await screen.findAllByRole('option');
      expect(options.map((o) => o.textContent?.trim())).toContain('Lowtide');
      await fireEvent.keyDown(box, { key: 'ArrowDown' });
      await fireEvent.submit(box.closest('form')!);
      await waitFor(() => expect(search.query).toBe(options[0]!.textContent!.trim()));
    } finally {
      vi.useRealTimers();
    }
  });

  it('the row menu offers Play next, Add to queue, Like, Add to playlist and Go to artist', async () => {
    await openApp();
    await typeAndSubmit('harbor');
    await fireEvent.click(await screen.findByRole('button', { name: 'More options for Neon Harbor' }));
    const dialog = await screen.findByRole('dialog', { name: 'Neon Harbor' });
    expect((screen.getByRole('button', { name: 'Add to playlist' }) as HTMLButtonElement).disabled).toBe(false);
    expect(dialog.textContent).toContain('Like');
    expect(dialog.textContent).toContain('Go to artist');
    await fireEvent.click(screen.getByRole('button', { name: 'Play next' }));
    await waitFor(() => expect(player.state.queue.map((e) => e.track.title)).toEqual(['Neon Harbor']));
  });

  it('Go to artist searches that artist\'s songs', async () => {
    await openApp();
    await typeAndSubmit('harbor');
    await fireEvent.click(await screen.findByRole('button', { name: 'More options for Neon Harbor' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Go to artist' }));
    await waitFor(() => expect(search.query).toBe('Lowtide'));
    expect(search.filter).toBe('songs');
  });

  it('filters switch the results, and an unsupported filter is disabled with an explanation', async () => {
    await openApp();
    await typeAndSubmit('lowtide');
    await screen.findByRole('button', { name: /^Neon Harbor/ });
    const albums = screen.getByRole('button', { name: 'Albums' }) as HTMLButtonElement;
    expect(albums.disabled).toBe(true);
    expect(albums.title).toMatch(/not available/i);
    await fireEvent.click(screen.getByRole('button', { name: 'Artists' }));
    await screen.findByText('Artist · show songs');
    expect(screen.getByRole('button', { name: 'Artists' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('explains failures, offers a retry, and recovers', async () => {
    await openApp();
    await typeAndSubmit('!offline');
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toMatch(/offline/i);
    expect(alert.textContent).toMatch(/connection/i);
    await fireEvent.input(screen.getByRole('combobox', { name: 'Search' }), { target: { value: 'lowtide' } });
    await fireEvent.submit(screen.getByRole('combobox', { name: 'Search' }).closest('form')!);
    await screen.findByRole('button', { name: /^Neon Harbor/ });
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('a missing yt-dlp is called out specifically; "no results" is not an error', async () => {
    await openApp();
    await typeAndSubmit('!missing');
    expect((await screen.findByRole('alert')).textContent).toMatch(/yt-dlp/);
    await typeAndSubmit('zzzz-no-match');
    expect(await screen.findByText('No results')).toBeTruthy();
    expect(screen.queryByRole('alert')).toBeNull();
  });
});

describe('queue screen', () => {
  async function withQueue() {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: /^Neon Harbor/ }));
    await waitFor(() => expect(player.state.queue.length).toBeGreaterThan(3));
    mock.tick(600);
    await fireEvent.click(screen.getByRole('button', { name: /Queue/ }));
    await screen.findByRole('list', { name: 'Queue' });
  }
  const titles = () => player.state.queue.map((e) => e.track.title);

  it('lists the queue and marks what is playing', async () => {
    await withQueue();
    const rows = within(screen.getByRole('list', { name: 'Queue' })).getAllByRole('listitem');
    expect(rows).toHaveLength(player.state.queue.length);
    expect(rows[0]!.textContent).toContain('Now playing');
    expect(screen.getByRole('heading', { name: 'Queue' })).toBeTruthy();
    expect(screen.getByText(`${player.state.queue.length} songs`)).toBeTruthy();
  });

  it('move buttons reorder, and the first and last rows cannot move out of range', async () => {
    await withQueue();
    expect((screen.getByRole('button', { name: 'Move Neon Harbor up' }) as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(screen.getByRole('button', { name: 'Move Neon Harbor down' }));
    await waitFor(() => expect(titles().slice(0, 2)).toEqual(['Paper Planes', 'Neon Harbor']));
  });

  it('dragging a row by its handle past other rows reorders the queue (pointer events, not HTML5 drag)', async () => {
    await withQueue();
    const items = within(screen.getByRole('list', { name: 'Queue' })).getAllByRole('listitem').filter((li) => li.hasAttribute('data-row'));
    // jsdom has no layout: give the rows 40px-high boxes, one under the other.
    items.forEach((li, i) => {
      li.getBoundingClientRect = () => ({ top: i * 40, height: 40, bottom: i * 40 + 40 }) as DOMRect;
    });
    const handle = within(items[0]!).getByTestId('drag-handle');
    handle.setPointerCapture = () => {};
    handle.releasePointerCapture = () => {};
    const fire = (type: string, y: number) =>
      handle.dispatchEvent(new PointerEvent(type, { pointerId: 7, clientY: y, button: 0, pointerType: 'mouse', bubbles: true, cancelable: true }));
    fire('pointerdown', 20);
    fire('pointermove', 105); // below the middle of the third row
    fire('pointerup', 105);
    await waitFor(() => expect(titles()[2]).toBe('Neon Harbor'));
  });

  it('clicking a row plays it; removing offers Undo', async () => {
    await withQueue();
    await fireEvent.click(screen.getByRole('button', { name: /^Slow Orbit/ }));
    await waitFor(() => expect(player.shown?.track.title).toBe('Slow Orbit'));
    const before = player.state.queue.length;
    await fireEvent.click(screen.getByRole('button', { name: 'Remove Glass Rooms from queue' }));
    await waitFor(() => expect(player.state.queue).toHaveLength(before - 1));
    snackbar.act(snackbar.items.at(-1)!.id);
    await waitFor(() => expect(player.state.queue).toHaveLength(before));
  });

  it('clearing empties the queue, shows the empty state, and Undo brings it back', async () => {
    await withQueue();
    const before = player.state.queue.length;
    await fireEvent.click(screen.getByRole('button', { name: 'Clear queue' }));
    await waitFor(() => expect(player.state.queue).toHaveLength(0));
    expect(await screen.findByText('Your queue is empty')).toBeTruthy();
    snackbar.act(snackbar.items.at(-1)!.id);
    await waitFor(() => expect(player.state.queue).toHaveLength(before));
  });

  it('"Save as playlist" opens a name prompt for the queue', async () => {
    await withQueue();
    const save = screen.getByRole('button', { name: 'Save as playlist' }) as HTMLButtonElement;
    expect(save.disabled).toBe(false);
    await fireEvent.click(save);
    expect(await screen.findByRole('dialog', { name: 'Save queue as playlist' })).toBeTruthy();
  });
});
