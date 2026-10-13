import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { DEMO_TRACKS, MockBackend } from '$lib/mock';
import { nav } from '$lib/nav.svelte';
import { player, EMPTY_STATE } from '$lib/player.svelte';
import { snackbar } from '$lib/snackbar.svelte';
import { search } from '$lib/search.svelte';
import { library } from '$lib/library.svelte';
import { theme } from '$lib/theme.svelte';
import { lyricsStore } from '$lib/lyrics.svelte';
import { sleep } from '$lib/sleep.svelte';
import { shellSettings } from '$lib/shell.svelte';

let mock: MockBackend;
vi.mock('$lib/backend', async (orig) => ({
  ...(await orig<typeof import('$lib/backend')>()),
  createBackend: async () => mock,
}));

import App from './App.svelte';
import Flyout from './views/Flyout.svelte';
import Mini from './views/Mini.svelte';

beforeEach(() => {
  mock = new MockBackend();
  mock.searchLatencyMs = 0;
  search.filter = 'all';
  player.state = EMPTY_STATE;
  player.ready = false;
  player.catalog = null;
  library.ready = false;
  nav.route = 'home';
  nav.nowPlayingOpen = false;
  nav.menuTrack = null;
  nav.themeSheetOpen = false;
  nav.sleepSheetOpen = false;
  nav.tab = 'upnext';
  lyricsStore.state = { kind: 'idle' };
  sleep.status = null;
  shellSettings.settings = { closeToTray: false, startWithWindows: false, toastOnChange: false };
  shellSettings.error = null;
  theme.settings = { followSystem: true, themeId: 'dark', lightId: 'light', darkId: 'dark', dynamicColor: false, custom: [] };
  for (const s of [...snackbar.items]) snackbar.dismiss(s.id);
  window.matchMedia = ((q: string) => ({
    matches: false, media: q, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, dispatchEvent: () => false, onchange: null,
  })) as typeof window.matchMedia;
});

async function playing() {
  render(App);
  await waitFor(() => expect(player.ready && library.ready).toBe(true));
  await screen.findByText(DEMO_TRACKS[0]!.title);
  await fireEvent.click(screen.getByRole('button', { name: /^Neon Harbor/ }));
  mock.tick(600);
  await screen.findByRole('button', { name: 'Pause' });
  await fireEvent.click(screen.getByRole('button', { name: 'Open Now Playing' }));
  await screen.findByRole('region', { name: 'Now Playing' });
}

async function lyricsTab() {
  await fireEvent.click(screen.getByRole('tab', { name: 'Lyrics' }));
}

describe('lyrics', () => {
  it('shows the lines of the song, highlights the one being sung and follows the position', async () => {
    await playing();
    await lyricsTab();
    const box = await screen.findByTestId('lyrics');
    await waitFor(() => expect(within(box).getAllByRole('button').length).toBeGreaterThan(2));
    const current = () => box.querySelector('[data-current="true"]')?.textContent;
    expect(current()).toBeUndefined(); // before the first line (it starts at 2 s; we are at 0.6 s)
    mock.tick(2_000); // 2.6 s
    await waitFor(() => expect(current()).toBe('Neon Harbor, line 1'));
    mock.tick(5_000); // 7.6 s
    await waitFor(() => expect(current()).toBe('Neon Harbor, line 2'));
  });

  it('is asked for once per song, not on every tab flip or position tick', async () => {
    await playing();
    await lyricsTab();
    await screen.findByTestId('lyrics');
    await waitFor(() => expect(mock.lyricsRequests).toEqual([DEMO_TRACKS[0]!.key]));
    await fireEvent.click(screen.getByRole('tab', { name: 'Up next' }));
    await lyricsTab();
    mock.tick(1_000);
    expect(mock.lyricsRequests).toHaveLength(1);
  });

  it('clicking a line seeks to it', async () => {
    await playing();
    await lyricsTab();
    const box = await screen.findByTestId('lyrics');
    const line = await within(box).findByRole('button', { name: 'Neon Harbor, line 3' });
    await fireEvent.click(line);
    await waitFor(() => expect(player.state.positionMs).toBe(12_000));
  });

  it('says so when a song has no lyrics, is instrumental, or the lookup failed (and the lookup can be retried)', async () => {
    await playing();
    mock.lyricsFor.set(DEMO_TRACKS[0]!.key, { kind: 'notFound' });
    await lyricsTab();
    expect(await screen.findByText('No lyrics found for this song.')).toBeTruthy();

    mock.lyricsFor.set(DEMO_TRACKS[0]!.key, { kind: 'error', message: 'could not reach the lyrics service' });
    await lyricsStore.retry(player.shown!.track);
    expect(await screen.findByRole('alert')).toBeTruthy();
    mock.lyricsFor.set(DEMO_TRACKS[0]!.key, { kind: 'plain', text: 'just words' });
    await fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByText('just words')).toBeTruthy();
    expect(screen.getByText(/not timed/)).toBeTruthy();

    mock.lyricsFor.set(DEMO_TRACKS[0]!.key, { kind: 'instrumental' });
    await lyricsStore.retry(player.shown!.track);
    expect(await screen.findByText(/instrumental/)).toBeTruthy();
  });
});

describe('sleep timer', () => {
  async function choose(label: string) {
    await fireEvent.click(screen.getByRole('button', { name: 'Sleep timer' }));
    const sheet = await screen.findByRole('dialog', { name: 'Sleep timer' });
    await fireEvent.click(within(sheet).getByRole('button', { name: label }));
  }

  it('a timer in minutes shows its countdown on Now Playing and pauses when it runs out', async () => {
    await playing();
    await choose('1 hour');
    const chip = await screen.findByTestId('sleep-chip');
    expect(chip.textContent).toBe('60 min left');
    mock.tick(30 * 60_000);
    await waitFor(() => expect(screen.getByTestId('sleep-chip').textContent).toBe('30 min left'));
    mock.tick(30 * 60_000 + 1_000);
    await waitFor(() => expect(screen.queryByTestId('sleep-chip')).toBeNull());
    await waitFor(() => expect(player.isPlaying).toBe(false));
  });

  it('can be extended by 15 minutes and turned off from its sheet', async () => {
    await playing();
    await choose('15 minutes');
    await fireEvent.click(screen.getByRole('button', { name: 'Sleep timer' }));
    const sheet = await screen.findByRole('dialog', { name: 'Sleep timer' });
    await fireEvent.click(within(sheet).getByRole('button', { name: '+15 min' }));
    await waitFor(() => expect(within(sheet).getByTestId('sleep-status').textContent).toBe('30 min left'));
    await fireEvent.click(within(sheet).getByRole('button', { name: 'Turn off' }));
    await waitFor(() => expect(sleep.status).toBeNull());
  });

  it('"End of this song" lets the song finish and then waits instead of starting the next', async () => {
    await playing();
    await choose('End of this song');
    expect((await screen.findByTestId('sleep-chip')).textContent).toBe('End of song');
    mock.tick(60_000); // the demo song is 38 s long
    await waitFor(() => expect(sleep.status).toBeNull());
    await waitFor(() => expect(player.isPlaying).toBe(false));
    expect(player.state.current).toBe(player.state.queue[1]!.id); // parked on the next song
  });

  it('choosing an option confirms with a snackbar that can turn it off', async () => {
    await playing();
    await choose('After 2 songs');
    const bar = await screen.findByText('Sleep timer: after 2 songs');
    expect(bar).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Turn off' }));
    await waitFor(() => expect(sleep.status).toBeNull());
  });
});

describe('Windows settings', () => {
  async function openSettings() {
    render(App);
    await waitFor(() => expect(player.ready && library.ready).toBe(true));
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    return screen.findByRole('region', { name: 'Windows' });
  }

  it('the three switches save through the backend', async () => {
    const section = await openSettings();
    await fireEvent.click(within(section).getByLabelText(/Keep playing in the tray/));
    await waitFor(() => expect(mock.shell.closeToTray).toBe(true));
    await fireEvent.click(within(section).getByLabelText(/Start Pyxee when I sign in/));
    await fireEvent.click(within(section).getByLabelText(/notification when the song changes/));
    await waitFor(() => expect(mock.shell).toEqual({ closeToTray: true, startWithWindows: true, toastOnChange: true }));
  });

  it('a switch that could not be saved goes back and says why', async () => {
    const section = await openSettings();
    mock.shellError = 'Windows refused';
    const box = within(section).getByLabelText(/Start Pyxee when I sign in/) as HTMLInputElement;
    await fireEvent.click(box);
    expect((await within(section).findByRole('alert')).textContent).toContain('Windows refused');
    await waitFor(() => expect(box.checked).toBe(false));
    expect(mock.shell.startWithWindows).toBe(false);
  });

  it('the sleep timer can be opened from Settings', async () => {
    const section = await openSettings();
    await fireEvent.click(within(section).getByRole('button', { name: /Sleep timer/ }));
    expect(await screen.findByRole('dialog', { name: 'Sleep timer' })).toBeTruthy();
  });
});

describe('tray flyout and mini player', () => {
  async function started(view: typeof Flyout | typeof Mini) {
    render(view);
    await waitFor(() => expect(player.ready).toBe(true));
  }

  it('the flyout shows the song, controls it, and Escape closes it', async () => {
    mock.start(); // not needed; ticks are driven by hand
    mock.stop();
    await mock.dispatch({ type: 'playTracks', tracks: DEMO_TRACKS.slice(0, 3), start: 0 });
    mock.tick(600);
    await started(Flyout);
    await waitFor(() => expect(screen.getByTestId('compact-title').textContent).toBe('Neon Harbor'));
    await fireEvent.click(screen.getByRole('button', { name: 'Pause' }));
    await waitFor(() => expect(player.isPlaying).toBe(false));
    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    await waitFor(() => expect(screen.getByTestId('compact-title').textContent).toBe('Paper Planes'));
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(mock.trayCalls).toContain('flyoutHide');
    await fireEvent.click(screen.getByRole('button', { name: 'Open Pyxee' }));
    expect(mock.trayCalls).toContain('flyoutOpenMain');
  });

  it('the flyout shows a thin progress line that follows the song', async () => {
    await mock.dispatch({ type: 'playTracks', tracks: DEMO_TRACKS.slice(0, 1), start: 0 });
    mock.tick(600);
    await started(Flyout);
    mock.tick(18_400);
    const expected = () => String(Math.round((player.state.positionMs / 38_000) * 100));
    await waitFor(() => expect(screen.getByRole('progressbar', { name: 'Progress' }).getAttribute('aria-valuenow')).toBe(expected()));
    expect(Number(expected())).toBeGreaterThan(40);
  });

  it('with nothing playing the flyout says so instead of showing an empty card', async () => {
    await started(Flyout);
    expect(screen.getByTestId('compact-title').textContent).toBe('Nothing playing');
  });

  it('the mini player is draggable, and its buttons open the app or close it', async () => {
    await mock.dispatch({ type: 'playTracks', tracks: DEMO_TRACKS.slice(0, 2), start: 0 });
    mock.tick(600);
    await started(Mini);
    expect(screen.getByTestId('mini').hasAttribute('data-tauri-drag-region')).toBe(true);
    await waitFor(() => expect(screen.getByTestId('compact-title').textContent).toBe('Neon Harbor'));
    const buttons = screen.getAllByRole('button', { name: 'Open Pyxee' });
    await fireEvent.click(buttons[buttons.length - 1]!);
    expect(mock.trayCalls).toContain('mainShow');
    await fireEvent.click(screen.getByRole('button', { name: 'Close mini player' }));
    expect(mock.trayCalls).toContain('miniToggle');
  });
});
