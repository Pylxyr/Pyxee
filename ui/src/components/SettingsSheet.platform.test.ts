import { afterEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import SettingsSheet from './SettingsSheet.svelte';
import { nav } from '$lib/nav.svelte';
import { platform } from '$lib/platform.svelte';

afterEach(() => {
  platform.os = 'windows';
  nav.themeSheetOpen = false;
});

describe('Settings wording by system', () => {
  it('says Windows on Windows, with the notification-area buttons', () => {
    nav.themeSheetOpen = true;
    render(SettingsSheet, { backend: null });
    expect(screen.getByRole('heading', { name: 'Windows' })).toBeTruthy();
    expect(screen.getByText('Start Pyxee when I sign in to Windows')).toBeTruthy();
    expect(screen.queryByText(/buttons in the notification area/)).toBeTruthy();
  });

  it('says System on Linux, with no Windows wording and no notification-area buttons', () => {
    platform.os = 'linux';
    nav.themeSheetOpen = true;
    const { container } = render(SettingsSheet, { backend: null });
    expect(screen.getByRole('heading', { name: 'System' })).toBeTruthy();
    expect(screen.getByText('Start Pyxee when I sign in')).toBeTruthy();
    expect(screen.queryByText(/buttons in the notification area/)).toBeNull();
    expect(container.textContent ?? '').not.toMatch(/Windows|SmartScreen|installer/);
  });
});
