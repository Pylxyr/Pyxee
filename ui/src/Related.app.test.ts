import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { DEMO_TRACKS, MockBackend } from '$lib/mock';
import { library } from '$lib/library.svelte';
import { nav } from '$lib/nav.svelte';
import { player, EMPTY_STATE } from '$lib/player.svelte';
import { snackbar } from '$lib/snackbar.svelte';

let mock: MockBackend;
vi.mock('$lib/backend', async (orig) => ({
  ...(await orig<typeof import('$lib/backend')>()),
  createBackend: async () => mock,
}));

import App from './App.svelte';

beforeEach(() => {
  mock = new MockBackend();
  mock.searchLatencyMs = 0;
  player.state = EMPTY_STATE;
  player.ready = false;
  player.catalog = null;
  library.ready = false;
  library.snapshot = { revision: 0, likedKeys: [], playlists: [], historyCount: 0, error: null };
  Object.assign(nav, { route: 'home', nowPlayingOpen: false, menuTrack: null, pickTrack: null, namePrompt: null, libraryTab: 'liked', playlistId: null, themeSheetOpen: false, tab: 'upnext' });
  for (const s of [...snackbar.items]) snackbar.dismiss(s.id);
});

async function openRelated() {
  render(App);
  await waitFor(() => expect(player.ready && library.ready).toBe(true));
  await fireEvent.click(await screen.findByRole('button', { name: /^Neon Harbor/ }));
  await fireEvent.click(await screen.findByRole('button', { name: 'Open Now Playing' }));
  const root = await screen.findByTestId('np-root');
  await fireEvent.click(within(root).getByRole('tab', { name: 'Related' }));
  return root;
}

describe('Now Playing > Related', () => {
  it('lists similar songs for the song being shown and plays one with the rest queued after it', async () => {
    const root = await openRelated();
    const panel = await within(root).findByTestId('related', {}, { timeout: 3000 });
    const list = await within(panel).findByRole('list', { name: 'Related songs' }, { timeout: 3000 });
    const rows = within(list).getAllByRole('listitem');
    expect(rows.length).toBeGreaterThan(0);
    expect(mock.relatedCalls).toHaveLength(1);
    expect(within(panel).queryByText(/arrive with search/)).toBeNull();
    await fireEvent.click(within(rows[0]!).getAllByRole('button')[0]!);
    expect(player.state.queue.length).toBeGreaterThan(1);
  });

  it('says what went wrong and offers Try again', async () => {
    mock.relatedFailure = { kind: 'offline', message: "You're offline." };
    const root = await openRelated();
    const msg = await within(root).findByText(/offline/i, {}, { timeout: 3000 });
    expect(msg.textContent).toContain("Couldn't load related songs");
    mock.relatedFailure = null;
    await fireEvent.click(within(root).getByRole('button', { name: 'Try again' }));
    await within(root).findByRole('list', { name: 'Related songs' }, { timeout: 3000 });
  });
});
