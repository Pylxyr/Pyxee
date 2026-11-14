<script lang="ts">
  import { onMount } from 'svelte';
  import type { WindowControls } from '$lib/windowControls';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  let { controls, showBrand = false }: { controls: WindowControls; /** The tabs already show the name in the wide layout. */ showBrand?: boolean } = $props();

  let maximized = $state(false);

  // Read the state once and again after every resize: maximizing and restoring are resizes.
  onMount(() => {
    let live = true;
    let unlisten: (() => void) | null = null;
    const sync = () => void controls.isMaximized().then((m) => live && (maximized = m)).catch(() => {});
    sync();
    controls.onResized(sync).then((off) => (live ? (unlisten = off) : off())).catch(() => {});
    return () => {
      live = false;
      unlisten?.();
    };
  });

  // A failed call (the window is going away) leaves nothing to tell the listener.
  const run = (f: () => Promise<void>) => void f().catch(() => {});
</script>

<!-- The empty part of the bar drags the window (and a double click maximizes it); the buttons are not part of the drag region. -->
<div class="titlebar" data-tauri-drag-region data-testid="titlebar">
  {#if showBrand}<span class="titlebar-brand pointer-events-none"><Logo size={14} /> Pyxee</span>{:else}<span></span>{/if}
  <div class="titlebar-buttons">
    <button class="caption-btn" aria-label="Minimize" title="Minimize" onclick={() => run(() => controls.minimize())}><Icon name="minimize" size={14} /></button>
    <button class="caption-btn" aria-label={maximized ? 'Restore' : 'Maximize'} title={maximized ? 'Restore' : 'Maximize'} onclick={() => run(() => controls.toggleMaximize())}><Icon name={maximized ? 'restore' : 'maximize'} size={14} /></button>
    <button class="caption-btn caption-close" aria-label="Close" title="Close" onclick={() => run(() => controls.close())}><Icon name="close" size={14} /></button>
  </div>
</div>
