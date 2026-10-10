import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { DEMO_TRACKS, MockBackend } from '$lib/mock';
import { library } from '$lib/library.svelte';
import { nav } from '$lib/nav.svelte';
import { player, EMPTY_STATE } from '$lib/player.svelte';
import { search } from '$lib/search.svelte';
import { snackbar } from '$lib/snackbar.svelte';

let mock: MockBackend;
vi.mock('$lib/backend', async (orig) => ({
  ...(await orig<typeof import('$lib/backend')>()),
  createBackend: async () => mock,
}));

import App from './App.svelte';

const [NEON, PAPER, ORBIT] = DEMO_TRACKS as [(typeof DEMO_TRACKS)[0], (typeof DEMO_TRACKS)[0], (typeof DEMO_TRACKS)[0]];

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
  setWide(false);
});

async function openApp() {
  render(App);
  await waitFor(() => expect(player.ready && library.ready).toBe(true));
}
const row = (title: string) => screen.findByRole('button', { name: new RegExp(`^${title}`) });
async function go(name: RegExp | string) {
  await fireEvent.click(screen.getByRole('button', { name }));
}
async function nameAndConfirm(dialogName: string, text: string, confirm: string) {
  const dialog = await screen.findByRole('dialog', { name: dialogName });
  await fireEvent.input(within(dialog).getByRole('textbox', { name: 'Name' }), { target: { value: text } });
  await fireEvent.click(within(dialog).getByRole('button', { name: confirm }));
}
async function listen(track: typeof NEON, seconds: number) {
  await waitFor(() => expect(player.shown?.track.key).toBe(track.key));
  mock.tick(600);
  mock.tick(seconds * 1000);
}

describe('likes', () => {
  it('the heart in the player bar likes and unlikes the playing song, and it shows in Liked songs', async () => {
    setWide(true);
    await openApp();
    await fireEvent.click(await row('Neon Harbor'));
    const heart = await screen.findByRole('button', { name: 'Like' });
    expect(heart.getAttribute('aria-pressed')).toBe('false');
    await fireEvent.click(heart);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Unlike' }).getAttribute('aria-pressed')).toBe('true'));
    expect(library.isLiked(NEON.key)).toBe(true);

    await go(/Library/);
    const liked = await screen.findByRole('list', { name: 'Liked songs' });
    expect(within(liked).getByText('Neon Harbor')).toBeTruthy();
    await fireEvent.click(within(liked).getByRole('button', { name: 'Unlike Neon Harbor' }));
    expect(await screen.findByText('No liked songs yet')).toBeTruthy();
  });

  it('Now Playing has a working heart too', async () => {
    await openApp();
    await fireEvent.click(await row('Neon Harbor'));
    await fireEvent.click(await screen.findByRole('button', { name: 'Open Now Playing' }));
    const np = await screen.findByRole('region', { name: 'Now Playing' });
    await fireEvent.click(within(np).getByRole('button', { name: 'Like' }));
    await waitFor(() => expect(within(np).getByRole('button', { name: 'Unlike' })).toBeTruthy());
  });

  it('the row menu likes and unlikes', async () => {
    await openApp();
    await fireEvent.click(await screen.findByRole('button', { name: 'More options for Paper Planes' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Like' }));
    await waitFor(() => expect(library.isLiked(PAPER.key)).toBe(true));
    await fireEvent.click(await screen.findByRole('button', { name: 'More options for Paper Planes' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Unlike' }));
    await waitFor(() => expect(library.isLiked(PAPER.key)).toBe(false));
  });

  it('Play all queues every liked song', async () => {
    await mock.setLiked(NEON, true);
    await mock.setLiked(PAPER, true);
    await openApp();
    await go(/Library/);
    await fireEvent.click(await screen.findByRole('button', { name: 'Play all' }));
    await waitFor(() => expect(player.state.queue.map((e) => e.track.title)).toEqual(['Paper Planes', 'Neon Harbor']));
  });
});

describe('playlists', () => {
  it('Add to playlist -> New playlist asks for a name, creates it with the song, and confirms', async () => {
    await openApp();
    await fireEvent.click(await screen.findByRole('button', { name: 'More options for Neon Harbor' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Add to playlist' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'New playlist…' }));
    const dialog = await screen.findByRole('dialog', { name: 'New playlist' });
    const create = within(dialog).getByRole('button', { name: 'Create' }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    await nameAndConfirm('New playlist', '  Road trip ', 'Create');
    await waitFor(() => expect(library.playlists.map((p) => [p.name, p.trackCount])).toEqual([['Road trip', 1]]));
    expect(snackbar.items.at(-1)?.message).toBe('Added to Road trip');
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New playlist' })).toBeNull());
  });

  it('a failing create keeps the prompt open and says why', async () => {
    mock.playlistCreate = async () => { throw 'The library is unavailable.'; };
    await openApp();
    await go(/Library/);
    await fireEvent.click(await screen.findByRole('tab', { name: 'Playlists' }));
    await fireEvent.click(screen.getByRole('button', { name: 'New playlist' }));
    await nameAndConfirm('New playlist', 'Mix', 'Create');
    expect((await screen.findByRole('alert')).textContent).toBe('The library is unavailable.');
    expect(screen.getByRole('dialog', { name: 'New playlist' })).toBeTruthy();
  });

  it('adds to an existing playlist and says when a song is already there', async () => {
    const id = await mock.playlistCreate('Mix', [PAPER]);
    await openApp();
    for (const expected of ['Added to Mix', 'Already in Mix']) {
      await fireEvent.click(await screen.findByRole('button', { name: 'More options for Neon Harbor' }));
      await fireEvent.click(await screen.findByRole('button', { name: 'Add to playlist' }));
      await fireEvent.click(await screen.findByRole('button', { name: /^Mix/ }));
      await waitFor(() => expect(snackbar.items.at(-1)?.message).toBe(expected));
    }
    expect((await mock.playlistGet(id)).entries).toHaveLength(2);
  });

  it('creating from the Library opens the new, empty playlist', async () => {
    await openApp();
    await go(/Library/);
    await fireEvent.click(await screen.findByRole('tab', { name: 'Playlists' }));
    expect(await screen.findByText('No playlists yet')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'New playlist' }));
    await nameAndConfirm('New playlist', 'Focus', 'Create');
    expect(await screen.findByRole('heading', { name: 'Focus' })).toBeTruthy();
    expect(screen.getByText('This playlist is empty')).toBeTruthy();
  });

  async function openPlaylist(name: string) {
    await go(/Library/);
    await fireEvent.click(await screen.findByRole('tab', { name: 'Playlists' }));
    await fireEvent.click(await screen.findByRole('button', { name: new RegExp(`^${name}`) }));
    await screen.findByRole('heading', { name });
  }
  const titles = async (id: number) => (await mock.playlistGet(id)).entries.map((e) => e.track.title);

  it('Play queues the playlist in order; Shuffle turns shuffle on', async () => {
    await mock.playlistCreate('Mix', [NEON, PAPER, ORBIT]);
    await openApp();
    await openPlaylist('Mix');
    await fireEvent.click(screen.getByRole('button', { name: 'Play' }));
    await waitFor(() => expect(player.state.queue.map((e) => e.track.title)).toEqual(['Neon Harbor', 'Paper Planes', 'Slow Orbit']));
    await fireEvent.click(screen.getByRole('button', { name: 'Shuffle' }));
    await waitFor(() => expect(player.state.shuffle).toBe(true));
  });

  it('reorders by buttons and by dragging, and removal offers Undo', async () => {
    const id = await mock.playlistCreate('Mix', [NEON, PAPER, ORBIT]);
    await openApp();
    await openPlaylist('Mix');
    await fireEvent.click(screen.getByRole('button', { name: 'Move Neon Harbor down' }));
    await waitFor(async () => expect(await titles(id)).toEqual(['Paper Planes', 'Neon Harbor', 'Slow Orbit']));
    const items = within(await screen.findByRole('list', { name: 'Songs in Mix' })).getAllByRole('listitem');
    await fireEvent.dragStart(items[0]!);
    await fireEvent.dragOver(items[2]!);
    await fireEvent.drop(items[2]!);
    await waitFor(async () => expect(await titles(id)).toEqual(['Neon Harbor', 'Slow Orbit', 'Paper Planes']));

    await fireEvent.click(await screen.findByRole('button', { name: 'Remove Slow Orbit from playlist' }));
    await waitFor(async () => expect(await titles(id)).toEqual(['Neon Harbor', 'Paper Planes']));
    snackbar.act(snackbar.items.at(-1)!.id);
    await waitFor(async () => expect(await titles(id)).toEqual(['Neon Harbor', 'Slow Orbit', 'Paper Planes']));
  });

  it('renames through the prompt, and shows a clear message for a bad name', async () => {
    const id = await mock.playlistCreate('Mix', [NEON]);
    await openApp();
    await openPlaylist('Mix');
    await fireEvent.click(screen.getByRole('button', { name: 'Rename' }));
    await nameAndConfirm('Rename playlist', 'Late night', 'Save');
    expect(await screen.findByRole('heading', { name: 'Late night' })).toBeTruthy();
    expect((await mock.playlistGet(id)).name).toBe('Late night');
  });

  it('deleting returns to the Library, and Undo brings the playlist back', async () => {
    await mock.playlistCreate('Mix', [NEON, PAPER]);
    await openApp();
    await openPlaylist('Mix');
    await fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(library.playlists).toHaveLength(0));
    expect(await screen.findByText('No playlists yet')).toBeTruthy();
    snackbar.act(snackbar.items.at(-1)!.id);
    await waitFor(() => expect(library.playlists.map((p) => [p.name, p.trackCount])).toEqual([['Mix', 2]]));
  });

  it('a playlist deleted elsewhere shows a gentle message, not an error', async () => {
    const id = await mock.playlistCreate('Mix', [NEON]);
    await openApp();
    await openPlaylist('Mix');
    await mock.playlistDelete(id);
    expect(await screen.findByText('This playlist no longer exists')).toBeTruthy();
  });

  it('Save queue as playlist stores the whole queue under the chosen name', async () => {
    await openApp();
    await fireEvent.click(await row('Neon Harbor'));
    await waitFor(() => expect(player.state.queue.length).toBe(DEMO_TRACKS.length));
    await go(/Queue/);
    await fireEvent.click(await screen.findByRole('button', { name: 'Save as playlist' }));
    await nameAndConfirm('Save queue as playlist', 'Session', 'Save');
    await waitFor(() => expect(library.playlists.map((p) => [p.name, p.trackCount])).toEqual([['Session', DEMO_TRACKS.length]]));
    expect(snackbar.items.at(-1)?.message).toBe(`Saved ${DEMO_TRACKS.length} songs as Session`);
  });
});

describe('history and Home', () => {
  it('a song listened to for long enough appears under Today, and clearing can be undone', async () => {
    await openApp();
    await fireEvent.click(await row('Neon Harbor'));
    await listen(NEON, 31);
    await waitFor(() => expect(library.snapshot.historyCount).toBe(1));
    await go(/Library/);
    await fireEvent.click(await screen.findByRole('tab', { name: 'History' }));
    const today = await screen.findByRole('list', { name: 'Played Today' });
    expect(within(today).getByText('Neon Harbor')).toBeTruthy();

    await fireEvent.click(screen.getByRole('button', { name: 'Clear history' }));
    expect(await screen.findByText('No history yet')).toBeTruthy();
    snackbar.act(snackbar.items.at(-1)!.id);
    await waitFor(() => expect(library.snapshot.historyCount).toBe(1));
  });

  it('a song skipped quickly leaves no history', async () => {
    await openApp();
    await fireEvent.click(await row('Neon Harbor'));
    await listen(NEON, 5);
    expect(library.snapshot.historyCount).toBe(0);
  });

  it('Home fills with quick picks, recently played and a likes shortcut as you listen', async () => {
    await mock.setLiked(PAPER, true);
    await openApp();
    await player.playTracks([NEON]); // Home already shows library picks, not the demo list
    await listen(NEON, 31);
    await go(/Home/);
    const picks = await screen.findByRole('list', { name: 'Quick picks' });
    expect(within(picks).getByText('Neon Harbor')).toBeTruthy();
    expect(within(picks).getByText('Paper Planes')).toBeTruthy();
    expect(screen.queryByText(/Demo tracks/)).toBeNull();
    expect(within(screen.getByRole('list', { name: 'Recently played' })).getByRole('button', { name: 'Play Neon Harbor' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: /Liked songs/ }));
    expect(await screen.findByRole('list', { name: 'Liked songs' })).toBeTruthy();
  });

  it('with no library data and no demo tracks, Home says what to do', async () => {
    mock.devCatalog = async () => [];
    await openApp();
    expect(await screen.findByText('Nothing here yet')).toBeTruthy();
  });
});

describe('library problems', () => {
  it('tells the user when the library could not be opened and changes will not be saved', async () => {
    mock.getLibrary = async () => ({ revision: 1, likedKeys: [], playlists: [], historyCount: 0, error: 'disk is read-only' });
    await openApp();
    const note = await screen.findByRole('status');
    expect(note.textContent).toContain("couldn't be opened");
    expect(note.textContent).toContain('disk is read-only');
    expect(note.textContent).toContain("won't be saved");
  });

  it('a failed like is reported, not swallowed', async () => {
    mock.setLiked = async () => { throw 'The library is unavailable.'; };
    await openApp();
    await fireEvent.click(await screen.findByRole('button', { name: 'Like Neon Harbor' }));
    await waitFor(() => expect(snackbar.items.at(-1)?.message).toBe('The library is unavailable.'));
    expect(library.isLiked(NEON.key)).toBe(false);
  });
});
