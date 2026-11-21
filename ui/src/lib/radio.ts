import type { TrackDto } from './bindings/TrackDto';
import { library } from './library.svelte';
import { player } from './player.svelte';
import { snackbar } from './snackbar.svelte';

/** How many similar songs are queued behind the one that was picked. */
export const RADIO_SIZE = 15;

// Only the latest pick may add to the queue: picking something else while the similar songs are still being found
// leaves the queue to the new pick.
let latest = 0;

/**
 * Plays `seed` at once and queues songs similar to it behind it ("a radio"), so one pick keeps playing by itself.
 * The song starts without waiting for the lookup; if the lookup fails the song plays on its own and a note says so.
 */
export async function playWithRelated(seed: TrackDto): Promise<void> {
  const mine = ++latest;
  await player.playTracks([seed], 0);
  const backend = library.backend;
  if (!backend) return;
  let found: TrackDto[];
  try {
    found = await backend.relatedGet(seed.key, RADIO_SIZE);
  } catch {
    if (mine === latest) snackbar.push("Playing just this song: similar songs aren't available right now");
    return;
  }
  const seen = new Set<string>([seed.key]);
  for (const track of found) {
    if (seen.has(track.key)) continue;
    seen.add(track.key);
    // Stop if another pick came in, or the person moved on to a different song, while these were being found.
    const shown = player.shown?.track.key;
    if (mine !== latest || (shown !== undefined && shown !== seed.key)) return;
    await player.addToQueue(track);
  }
}
