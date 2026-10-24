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
import { updates } from '$lib/updates.svelte';

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
  updates.reset();
  shellSettings.settings = { closeToTray: false, startWithWindows: false, toastOnChange: false, trayControls: false, flyoutOnClick: true, discordPresence: true, lastfmScrobbling: false, borderless: true };
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
    await waitFor(() => expect(mock.shell).toEqual({ closeToTray: true, startWithWindows: true, toastOnChange: true, trayControls: false, flyoutOnClick: true, discordPresence: true, lastfmScrobbling: false, borderless: true }));
  });

  it('the notification area switches say what they do and save through the backend', async () => {
    const section = await openSettings();
    const buttons = within(section).getByLabelText(/previous, play and next buttons in the notification area/i);
    const click = within(section).getByLabelText(/Clicking the Pyxee tray icon opens the mini controls/i);
    expect((buttons as HTMLInputElement).checked).toBe(false);
    expect((click as HTMLInputElement).checked).toBe(true);
    expect(within(section).getByText(/hidden-icons flyout/i)).toBeTruthy();
    await fireEvent.click(buttons);
    await waitFor(() => expect(mock.shell.trayControls).toBe(true));
    await fireEvent.click(click);
    await waitFor(() => expect(mock.shell.flyoutOnClick).toBe(false));
  });

  it('the Discord switch is on by default and saves through the backend', async () => {
    const section = await openSettings();
    const discord = within(section).getByLabelText(/playing on Discord/i);
    expect((discord as HTMLInputElement).checked).toBe(true);
    await fireEvent.click(discord);
    await waitFor(() => expect(mock.shell.discordPresence).toBe(false));
    await fireEvent.click(discord);
    await waitFor(() => expect(mock.shell.discordPresence).toBe(true));
  });

  it('the Borderless window switch is on by default and saves through the backend', async () => {
    const section = await openSettings();
    const borderless = within(section).getByLabelText(/Borderless window/i);
    expect((borderless as HTMLInputElement).checked).toBe(true);
    await fireEvent.click(borderless);
    await waitFor(() => expect(mock.shell.borderless).toBe(false));
    await fireEvent.click(borderless);
    await waitFor(() => expect(mock.shell.borderless).toBe(true));
  });

  it('on a narrow window Settings is a tab that slides in from the right, and closes with its button, Escape or a click outside', async () => {
    await openSettings();
    const tab = screen.getByRole('dialog', { name: 'Settings' });
    expect(tab.getAttribute('data-side')).toBe('right');
    expect(tab.className).toContain('w-1/2');
    await fireEvent.click(within(tab).getByRole('button', { name: 'Close Settings' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Settings' })).toBeNull());
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    await fireEvent.keyDown(await screen.findByRole('dialog', { name: 'Settings' }), { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Settings' })).toBeNull());
  });

  it('on a wide window Settings slides in from the left, beside the rail button that opens it', async () => {
    window.matchMedia = ((q: string) => ({
      matches: q.includes('min-width'), media: q, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, dispatchEvent: () => false, onchange: null,
    })) as typeof window.matchMedia;
    await openSettings();
    const tab = screen.getByRole('dialog', { name: 'Settings' });
    expect(tab.getAttribute('data-side')).toBe('left');
    expect(tab.className).toContain('w-1/2');
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

describe('Updates settings', () => {
  async function openUpdates() {
    render(App);
    await waitFor(() => expect(player.ready && library.ready).toBe(true));
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    return screen.findByRole('region', { name: 'Updates' });
  }

  it('says when Pyxee is up to date', async () => {
    const section = await openUpdates();
    await fireEvent.click(within(section).getByRole('button', { name: 'Check for updates' }));
    await waitFor(() => expect(within(section).getByTestId('app-update-status').textContent).toContain('latest version (0.1.0)'));
    expect(within(section).queryByRole('button', { name: /Download and install/ })).toBeNull();
  });

  it('offers the install only for a newer release that can be verified, and installs through the backend', async () => {
    const section = await openUpdates();
    mock.appUpdate = { current: '0.1.0', latest: '0.2.0', available: true, notesUrl: 'https://example.test/r', notes: '', installable: true, portable: false };
    await fireEvent.click(within(section).getByRole('button', { name: 'Check for updates' }));
    await fireEvent.click(await within(section).findByRole('button', { name: 'Download and install 0.2.0' }));
    await waitFor(() => expect(mock.updateCalls.filter((c) => c !== 'checkApp')).toEqual(['installApp']));
  });

  it('a release without an installer and checksums points to its page instead of offering the install', async () => {
    const section = await openUpdates();
    mock.appUpdate = { current: '0.1.0', latest: '0.2.0', available: true, notesUrl: 'https://example.test/r', notes: '', installable: false, portable: false };
    await fireEvent.click(within(section).getByRole('button', { name: 'Check for updates' }));
    await waitFor(() => expect(within(section).getByText(/https:\/\/example.test\/r/)).toBeTruthy());
    expect(within(section).queryByRole('button', { name: /Download and install/ })).toBeNull();
  });

  it('a portable copy is told to download the zip, never offered the installer', async () => {
    const section = await openUpdates();
    mock.appUpdate = { current: '0.1.0', latest: '0.2.0', available: true, notesUrl: 'https://example.test/r', notes: '', installable: false, portable: true };
    await fireEvent.click(within(section).getByRole('button', { name: 'Check for updates' }));
    expect((await within(section).findByTestId('portable-update')).textContent).toContain('portable copy');
    expect(within(section).queryByRole('button', { name: /Download and install/ })).toBeNull();
  });

  it('a failed check shows why and can be tried again', async () => {
    mock.updateFailure = 'GitHub is limiting requests right now.';
    const section = await openUpdates();
    await fireEvent.click(within(section).getByRole('button', { name: 'Check for updates' }));
    expect((await within(section).findByText(/limiting requests/)).getAttribute('role')).toBe('alert');
    expect((within(section).getByRole('button', { name: 'Check for updates' }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('updates yt-dlp when a newer one exists and says it is used from the next search', async () => {
    mock.ytdlpUpdate = { available: true, currentVersion: '2026.01.01' };
    const section = await openUpdates();
    await fireEvent.click(within(section).getByRole('button', { name: 'Check yt-dlp' }));
    await fireEvent.click(await within(section).findByRole('button', { name: 'Update yt-dlp' }));
    await waitFor(() => expect(within(section).getByTestId('ytdlp-update-status').textContent).toContain('Updated to 2026.10.01'));
    expect(within(section).queryByRole('button', { name: 'Update yt-dlp' })).toBeNull();
  });

  it('a yt-dlp update that fails says why and leaves the button to retry', async () => {
    mock.ytdlpUpdate = { available: true, currentVersion: null };
    const section = await openUpdates();
    await fireEvent.click(within(section).getByRole('button', { name: 'Check yt-dlp' }));
    mock.updateFailure = 'The download does not match its published checksum.';
    await fireEvent.click(await within(section).findByRole('button', { name: 'Update yt-dlp' }));
    expect(await within(section).findByText(/does not match its published checksum/)).toBeTruthy();
    expect(within(section).getByRole('button', { name: 'Update yt-dlp' })).toBeTruthy();
  });

  const NEWER = { current: '0.1.0', latest: '0.2.0', available: true, notesUrl: 'https://example.test/r', notes: "## What's new\n- Faster search\n- **Fixed** a skip bug", installable: true, portable: false };

  it('shows the changelog at once when the start-up check finds a newer version', async () => {
    mock.appUpdate = NEWER;
    render(App);
    const dialog = await screen.findByRole('dialog', { name: "What's new" });
    expect(within(dialog).getByTestId('changelog-version').textContent).toContain('0.2.0');
    expect(within(dialog).getByText('Faster search')).toBeTruthy();
    expect(within(dialog).getByText('Fixed a skip bug')).toBeTruthy();
    expect(mock.updateCalls).toEqual(['checkApp']);
  });

  it('stays quiet at start-up when Pyxee is up to date or the check fails', async () => {
    render(App);
    await waitFor(() => expect(mock.updateCalls).toEqual(['checkApp']));
    expect(screen.queryByRole('dialog', { name: "What's new" })).toBeNull();
    expect(updates.error).toBeNull();
  });

  it('a failed start-up check shows no error', async () => {
    mock.updateFailure = 'offline';
    render(App);
    await waitFor(() => expect(mock.updateCalls).toEqual(['checkApp']));
    await waitFor(() => expect(updates.phase).toBe('idle'));
    expect(screen.queryByRole('alert')).toBeNull();
    expect(screen.queryByRole('dialog', { name: "What's new" })).toBeNull();
  });

  it('shows the changelog when "Check for updates" finds a newer version, and "Later" closes it', async () => {
    const section = await openUpdates();
    mock.appUpdate = NEWER;
    await fireEvent.click(within(section).getByRole('button', { name: 'Check for updates' }));
    const dialog = await screen.findByRole('dialog', { name: "What's new" });
    expect(within(dialog).getByText('Faster search')).toBeTruthy();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Later' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: "What's new" })).toBeNull());
    // It can be read again from the Updates section.
    await fireEvent.click(within(section).getByRole('button', { name: "What's new" }));
    expect(await screen.findByRole('dialog', { name: "What's new" })).toBeTruthy();
  });

  it('installs from the changelog through the backend', async () => {
    mock.appUpdate = NEWER;
    render(App);
    const dialog = await screen.findByRole('dialog', { name: "What's new" });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Download and install 0.2.0' }));
    await waitFor(() => expect(mock.updateCalls).toContain('installApp'));
  });

  it('says so when the release has no written changelog', async () => {
    mock.appUpdate = { ...NEWER, notes: '' };
    render(App);
    const dialog = await screen.findByRole('dialog', { name: "What's new" });
    expect(within(dialog).getByText('This release has no written changelog.')).toBeTruthy();
  });

  it('does not open a changelog when the check finds nothing newer', async () => {
    const section = await openUpdates();
    await fireEvent.click(within(section).getByRole('button', { name: 'Check for updates' }));
    await waitFor(() => expect(within(section).getByTestId('app-update-status').textContent).toContain('latest version'));
    expect(screen.queryByRole('dialog', { name: "What's new" })).toBeNull();
  });
});

describe('Advanced settings', () => {
  async function openSettings() {
    render(App);
    await waitFor(() => expect(player.ready && library.ready).toBe(true));
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    return screen.findByRole('region', { name: 'Advanced' });
  }

  it('is closed by default: no settings are shown and nothing is read', async () => {
    const section = await openSettings();
    const toggle = within(section).getByRole('button', { name: 'Advanced' });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(within(section).queryByTestId('env-path')).toBeNull();
    expect(within(section).queryByRole('button', { name: 'Save' })).toBeNull();
  });

  it('opens and closes with its arrow button', async () => {
    const section = await openSettings();
    const toggle = within(section).getByRole('button', { name: 'Advanced' });
    await fireEvent.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    expect((await within(section).findByTestId('env-path')).textContent).toContain('.env');
    expect(within(section).getByRole('button', { name: 'Save' })).toBeTruthy();
    await fireEvent.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(within(section).queryByTestId('env-path')).toBeNull();
  });
});
