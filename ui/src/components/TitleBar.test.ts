import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import type { WindowControls } from '$lib/windowControls';
import TitleBar from './TitleBar.svelte';

function fakeControls(maximized = false) {
  let resized: () => void = () => {};
  const state = { maximized };
  const controls: WindowControls = {
    minimize: vi.fn(async () => {}),
    toggleMaximize: vi.fn(async () => {
      state.maximized = !state.maximized;
      resized();
    }),
    close: vi.fn(async () => {}),
    isMaximized: vi.fn(async () => state.maximized),
    isFullscreen: vi.fn(async () => false),
    setFullscreen: vi.fn(async () => {}),
    onResized: vi.fn(async (cb: () => void) => {
      resized = cb;
      return () => {};
    }),
  };
  return controls;
}

describe('TitleBar', () => {
  it('is a drag region, and its buttons call the window', async () => {
    const controls = fakeControls();
    render(TitleBar, { controls });
    expect(screen.getByTestId('titlebar').hasAttribute('data-tauri-drag-region')).toBe(true);
    await fireEvent.click(screen.getByRole('button', { name: 'Minimize' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(controls.minimize).toHaveBeenCalledOnce();
    expect(controls.close).toHaveBeenCalledOnce();
  });

  it('offers Restore while maximized and follows the window when it changes', async () => {
    const controls = fakeControls(true);
    render(TitleBar, { controls });
    await screen.findByRole('button', { name: 'Restore' });
    await fireEvent.click(screen.getByRole('button', { name: 'Restore' }));
    expect(controls.toggleMaximize).toHaveBeenCalledOnce();
    await waitFor(() => expect(screen.getByRole('button', { name: 'Maximize' })).toBeTruthy());
  });

  it('shows the name only when asked (the tabs show it in the wide layout)', () => {
    const { unmount } = render(TitleBar, { controls: fakeControls() });
    expect(screen.queryByText('Pyxee')).toBeNull();
    unmount();
    render(TitleBar, { controls: fakeControls(), showBrand: true });
    expect(screen.getByText('Pyxee')).toBeTruthy();
  });
});
