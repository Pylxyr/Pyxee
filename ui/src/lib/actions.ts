import type { EntryDto } from './bindings/EntryDto';
import { player } from './player.svelte';
import { snackbar } from './snackbar.svelte';

/** Removes a queue entry and offers Undo (in-page, never a native confirm). */
export function removeFromQueue(e: EntryDto) {
  player.remove(e.id);
  snackbar.push(`Removed "${e.track.title}"`, { action: { label: 'Undo', run: () => player.undo() } });
}

export function clearQueue() {
  if (player.state.queue.length === 0) return;
  player.dispatch({ type: 'clear' });
  snackbar.push('Queue cleared', { action: { label: 'Undo', run: () => player.undo() } });
}
