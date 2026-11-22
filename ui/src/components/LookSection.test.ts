import { beforeEach, describe, expect, it } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import LookSection from './LookSection.svelte';
import { look } from '$lib/look.svelte';
import { seekMode } from '$lib/seekmode.svelte';

beforeEach(() => {
  localStorage.clear();
  look.reset();
});

describe('LookSection', () => {
  it('offers every choice and applies one when it is picked', async () => {
    render(LookSection);
    for (const name of ['Corners', 'Surfaces', 'Panels', 'Cover shape', 'Home top card and Now Playing', 'Home: Tune in cards', 'Progress bar']) {
      expect(screen.getByRole('group', { name })).toBeTruthy();
    }
    await fireEvent.click(within(screen.getByRole('group', { name: 'Corners' })).getByRole('button', { name: 'Square' }));
    expect(look.value.corners).toBe('square');
  });
  it('switches the hero look and the progress bar, and resets', async () => {
    render(LookSection);
    await fireEvent.click(screen.getByRole('button', { name: 'Vinyl' }));
    expect(look.value.hero).toBe('vinyl');
    await fireEvent.click(screen.getByRole('button', { name: 'Steps' }));
    expect(seekMode.value).toBe('steps');
    await fireEvent.click(screen.getByRole('button', { name: 'Reset the look' }));
    expect(look.value.hero).toBe('blur');
  });
  it('says Now Playing stays plain only while Surfaces is Flat', async () => {
    render(LookSection);
    expect(screen.queryByText(/Now Playing stays plain/)).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Flat' }));
    expect(screen.getByText(/Now Playing stays plain/)).toBeTruthy();
  });
});
