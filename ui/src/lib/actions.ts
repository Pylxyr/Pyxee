import type { EntryDto } from './bindings/EntryDto';
import { player } from './player.svelte';
import { snackbar } from './snackbar.svelte';
import type { MenuExtra } from './nav.svelte';

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

/**
 * The lines the song menu of a queue entry adds (they stand in for Play now / Play next / Add to queue, which would put a
 * second copy of the song in the queue): play it, bring it up next, send it to the end, take it out.
 */
export function queueMenu(e: EntryDto): MenuExtra[] {
  const { queue, current } = player.state;
  const at = queue.findIndex((x) => x.id === e.id);
  const now = queue.findIndex((x) => x.id === current);
  const isCurrent = e.id === current;
  // The place right after the song that plays now (a song before it leaves a gap when it moves, so it lands one earlier).
  const next = now < 0 ? 0 : at < now ? now : now + 1;
  const out: MenuExtra[] = [{ label: isCurrent ? 'Play from the start' : 'Play now', icon: 'play', run: () => void player.playEntry(e.id) }];
  if (!isCurrent && at !== next) out.push({ label: 'Play next', icon: 'top', run: () => player.dispatch({ type: 'move', id: e.id, to: next }) });
  if (at !== queue.length - 1) out.push({ label: 'Move to the end', icon: 'down', run: () => player.dispatch({ type: 'move', id: e.id, to: queue.length - 1 }) });
  out.push({ label: 'Remove from queue', icon: 'trash', danger: true, run: () => removeFromQueue(e) });
  return out;
}
