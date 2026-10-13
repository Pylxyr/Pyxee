import { createBackend, type Backend } from '$lib/backend';
import { player } from '$lib/player.svelte';
import { theme } from '$lib/theme.svelte';

/** What the small windows (flyout, mini player) need: the player state and the same theme as the main window. */
export async function bootSmallWindow(): Promise<Backend> {
  const backend = await createBackend();
  await player.init(backend);
  theme.backdrop = (await backend.windowInfo()).backdrop;
  // The main window saves theme changes to localStorage; this window is another page of the same origin.
  window.addEventListener('storage', (e) => {
    if (e.key === 'pyxee.theme') theme.reload();
  });
  return backend;
}
