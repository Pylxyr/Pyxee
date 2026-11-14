import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { MockBackend } from '$lib/mock';
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
  player.state = EMPTY_STATE;
  player.ready = false;
  player.catalog = null;
  library.ready = false;
  nav.themeSheetOpen = false;
  for (const s of [...snackbar.items]) snackbar.dismiss(s.id);
});

async function openApp() {
  render(App);
  await waitFor(() => expect(player.ready && library.ready).toBe(true));
}

const sheetOf = (name: string) => screen.getByRole('dialog', { name }).closest('[data-sheet]') as HTMLElement;
const zOf = (el: HTMLElement) => Number(el.style.zIndex);

describe('sheets are above everything, and stack in the order they were opened', () => {
  it('the Settings button is a gear and opens a sheet called Settings with Appearance and Diagnostics', async () => {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    expect(await screen.findByRole('dialog', { name: 'Settings' })).toBeTruthy();
    expect(screen.getByRole('region', { name: 'Appearance' })).toBeTruthy();
    expect(screen.getByRole('region', { name: 'Diagnostics' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Appearance' })).toBeNull();
  });

  it('a sheet opened from Settings (the theme editor) lands on top of it, not underneath', async () => {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    await screen.findByRole('dialog', { name: 'Settings' });
    await fireEvent.click(screen.getByRole('button', { name: /Customize/ }));
    await screen.findByRole('dialog', { name: 'Customize theme' });
    // In the page the editor comes before the Settings sheet; only the opening order may decide.
    expect(zOf(sheetOf('Customize theme'))).toBeGreaterThan(zOf(sheetOf('Settings')));
  });

  it('every sheet is above Now Playing (z-40), and the snackbar above the sheets', async () => {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    await screen.findByRole('dialog', { name: 'Settings' });
    expect(zOf(sheetOf('Settings'))).toBeGreaterThan(40);
    const snackbarLayer = document.querySelector('[aria-live="polite"]') as HTMLElement;
    expect(snackbarLayer.className).toContain('z-[1000]');
  });
});

describe('messages from the engine', () => {
  it('a skipped-song notice appears as a message the listener can read', async () => {
    await openApp();
    mock.emitNotice('\u201cGone Song\u201d can\u2019t be played, so it was skipped: it isn\u2019t available. The log has details.');
    await waitFor(() => expect(snackbar.items.at(-1)?.message).toContain('Gone Song'));
    expect(snackbar.items.at(-1)?.message).toContain('The log has details');
    expect(await screen.findByText(/Gone Song/)).toBeTruthy();
  });

  it('several notices in a row are all shown (up to three at once)', async () => {
    await openApp();
    for (const n of ['one', 'two', 'three', 'four']) mock.emitNotice(n);
    await waitFor(() => expect(snackbar.items.map((i) => i.message)).toEqual(['two', 'three', 'four']));
  });
});

describe('Diagnostics (where the log is)', () => {
  async function openAppearance() {
    await openApp();
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    return screen.findByRole('region', { name: 'Diagnostics' });
  }

  it('shows the log file path and explains what the log is for', async () => {
    const section = await openAppearance();
    expect((await screen.findByTestId('log-path')).textContent).toContain('pyxee.log');
    expect(section.textContent).toContain('keeps skipping');
  });

  it('"Open log folder" asks the app to open it', async () => {
    await openAppearance();
    await fireEvent.click(await screen.findByRole('button', { name: 'Open log folder' }));
    await waitFor(() => expect(mock.logFolderOpens).toBe(1));
  });

  it('a problem creating the log file is shown, not hidden', async () => {
    mock.logError = 'could not open the log file: access denied';
    await openAppearance();
    expect((await screen.findByRole('alert')).textContent).toContain('access denied');
  });

  it('copying the path confirms, or says what to do instead if the clipboard is unavailable', async () => {
    await openAppearance();
    await screen.findByTestId('log-path');
    await fireEvent.click(screen.getByRole('button', { name: 'Copy path' }));
    await waitFor(() => expect(snackbar.items.at(-1)?.message).toMatch(/copied|Could not copy/));
    const written: string[] = [];
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: async (t: string) => void written.push(t) } });
    await fireEvent.click(screen.getByRole('button', { name: 'Copy path' }));
    await waitFor(() => expect(written[0]).toContain('pyxee.log'));
    expect(snackbar.items.at(-1)?.message).toBe('Log file path copied');
  });
});
