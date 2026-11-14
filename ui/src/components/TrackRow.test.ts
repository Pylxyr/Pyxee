import { afterEach, describe, expect, it } from 'vitest';
import { createEvent, fireEvent, render } from '@testing-library/svelte';
import TrackRow from './TrackRow.svelte';
import EditableRow from './EditableRow.svelte';
import { createRawSnippet } from 'svelte';
import { nav } from '$lib/nav.svelte';
import { DEMO_TRACKS } from '$lib/mock';

afterEach(() => {
  nav.menuTrack = null;
});

describe('right-clicking a song', () => {
  it('opens the same menu as its "more" button, and not the browser\'s', async () => {
    const track = DEMO_TRACKS[0]!;
    const { container } = render(TrackRow, { track, onplay: () => {} });
    const row = container.querySelector('li')!;
    const event = createEvent.contextMenu(row);
    await fireEvent(row, event);
    expect(event.defaultPrevented).toBe(true);
    expect(nav.menuTrack).toEqual(track);
  });

  it('does the same on a row of the queue, a playlist or Up next', async () => {
    const track = DEMO_TRACKS[1]!;
    const actions = createRawSnippet(() => ({ render: () => '<span></span>' }));
    const { container } = render(EditableRow, { track, onplay: () => {}, actions });
    const row = container.querySelector('li')!;
    const event = createEvent.contextMenu(row.querySelector('button')!);
    await fireEvent(row.querySelector('button')!, event);
    expect(event.defaultPrevented).toBe(true);
    expect(nav.menuTrack).toEqual(track);
  });
});
