<script lang="ts">
  import { theme } from '$lib/theme.svelte';
  import type { Backend } from '$lib/backend';
  import { bootSmallWindow } from './Boot';
  import Compact from './Compact.svelte';
  import Icon from '../components/Icon.svelte';

  const booted: Promise<Backend> = bootSmallWindow();
  const withBackend = (f: (b: Backend) => Promise<void>) => void booted.then(f);
  $effect(() => theme.apply(document.documentElement));
</script>

<!-- The whole card drags the window; the buttons stay clickable because they are not part of the drag region. -->
<div class="relative flex h-dvh items-center bg-bg p-3 text-fg" data-tauri-drag-region data-testid="mini">
  <Compact cover={72} onopen={() => withBackend((b) => b.mainShow())} />
  <div class="absolute right-1 top-1 flex">
    <button class="p-1 text-muted hover:text-fg" aria-label="Open Pyxee" title="Open Pyxee" onclick={() => withBackend((b) => b.mainShow())}><Icon name="expand" size={16} /></button>
    <button class="p-1 text-muted hover:text-fg" aria-label="Close mini player" title="Close" onclick={() => withBackend((b) => b.miniToggle())}><Icon name="close" size={16} /></button>
  </div>
</div>
