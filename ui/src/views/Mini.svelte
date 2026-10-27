<script lang="ts">
  import { followArt, theme } from '$lib/theme.svelte';
  import { player } from '$lib/player.svelte';
  import type { Backend } from '$lib/backend';
  import { bootSmallWindow } from './Boot';
  import Compact from './Compact.svelte';
  import IconButton from '../components/IconButton.svelte';

  const booted: Promise<Backend> = bootSmallWindow();
  const withBackend = (f: (b: Backend) => Promise<void>) => void booted.then(f);
  $effect(() => theme.apply(document.documentElement));
  // "Color from album art" in these windows too: they read the same setting and the cover of the song that plays.
  let backend = $state.raw<Backend | null>(null);
  void booted.then((b) => (backend = b));
  followArt(() => player.shown?.track.thumbnail ?? null, () => backend);
  // This window's own border takes the cover's colour too.
  $effect(() => void backend?.setWindowTint(theme.nativeTint));
</script>

<!-- The whole card drags the window; the buttons stay clickable because they are not part of the drag region. -->
<div class="relative flex h-dvh items-center p-3 text-fg" style="background:linear-gradient(120deg, var(--stage-wash), transparent 65%), var(--bg)" data-tauri-drag-region data-testid="mini">
  <Compact cover={72} onopen={() => withBackend((b) => b.mainShow())} />
  <div class="absolute right-1 top-1 flex">
    <IconButton icon="expand" iconSize={16} class="!h-6 !w-6" label="Open Pyxee" title="Open Pyxee" onclick={() => withBackend((b) => b.mainShow())} />
    <IconButton icon="close" iconSize={16} class="!h-6 !w-6" label="Close mini player" title="Close" onclick={() => withBackend((b) => b.miniToggle())} />
  </div>
</div>
