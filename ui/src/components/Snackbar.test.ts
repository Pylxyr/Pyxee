import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { snackbar } from '$lib/snackbar.svelte';
import Snackbar from './Snackbar.svelte';

beforeEach(() => {
  vi.useFakeTimers();
  for (const s of [...snackbar.items]) snackbar.dismiss(s.id);
});
afterEach(() => vi.useRealTimers());

describe('Snackbar', () => {
  it('every toast has a Dismiss button that closes just that toast, with or without an action', async () => {
    snackbar.push('Plain message');
    snackbar.push('Removed', { action: { label: 'Undo', run: () => {} } });
    render(Snackbar);
    const closers = await screen.findAllByRole('button', { name: 'Dismiss' });
    expect(closers).toHaveLength(2);
    await fireEvent.click(closers[0]!);
    expect(snackbar.items.map((s) => s.message)).toEqual(['Removed']);
  });

  it('Escape inside a toast closes it', async () => {
    snackbar.push('Something happened');
    const { container } = render(Snackbar);
    const toast = container.querySelector('.toast')!;
    await fireEvent.keyDown(toast, { key: 'Escape' });
    expect(snackbar.items).toHaveLength(0);
  });
});
