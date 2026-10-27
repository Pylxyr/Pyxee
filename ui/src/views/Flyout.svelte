<script lang="ts">
  import { followArt, theme } from '$lib/theme.svelte';
  import { player } from '$lib/player.svelte';
  import type { Backend } from '$lib/backend';
  import { bootSmallWindow } from './Boot';
  import Compact from './Compact.svelte';

  // Started at once; a click that comes before it is ready simply waits for it.
  const booted: Promise<Backend> = bootSmallWindow();
  const withBackend = (f: (b: Backend) => Promise<void>) => void booted.then(f);
  $effect(() => theme.apply(document.documentElement));
  // "Color from album art" in these windows too: they read the same setting and the cover of the song that plays.
  let backend = $state.raw<Backend | null>(null);
  void booted.then((b) => (backend = b));
  followArt(() => player.shown?.track.thumbnail ?? null, () => backend);
  // This window's own border takes the cover's colour too.
  $effect(() => void backend?.setWindowTint(theme.nativeTint));

  // Esc closes it; so does clicking anywhere else (the window hides itself when it loses focus).
  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') withBackend((b) => b.flyoutHide());
  }
</script>

<svelte:window {onkeydown} />

<div class="flex h-dvh items-center p-3 text-fg" style="background:linear-gradient(120deg, var(--stage-wash), transparent 65%), var(--bg)" data-testid="flyout">
  <Compact cover={96} onopen={() => withBackend((b) => b.flyoutOpenMain())} />
</div>
