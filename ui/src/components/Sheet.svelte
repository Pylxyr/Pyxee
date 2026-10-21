<script module lang="ts">
  // Sheets stack in the order they were opened, not in the order they appear in the page: a sheet
  // opened from another sheet (e.g. the theme editor from Settings) must land on top of it.
  // 100 and up keeps every sheet above Now Playing (z-40), the dock, the rail and the top bar;
  // the snackbar sits above all of them so its Undo stays reachable.
  let opened = 0;
</script>

<script lang="ts">
  import { fly, fade } from 'svelte/transition';
  import type { Snippet } from 'svelte';
  import { MOTION, motionMs, motionPx } from '$lib/motion';
  import IconButton from './IconButton.svelte';

  // `bottom` (the default) is a card that rises from the bottom, centred on wide windows. `right` is a tab that
  // slides in from the right edge and takes half of the window (all of it when the window is narrow); Settings uses it.
  let { open, title, onclose, children, side = 'bottom' }: { open: boolean; title: string; onclose: () => void; children: Snippet; side?: 'bottom' | 'right' } = $props();
  const right = $derived(side === 'right');
  let panel = $state<HTMLElement>();
  let restore: HTMLElement | null = null;
  let z = $state(100);

  // Before the sheet is drawn, so it never appears for a frame at the wrong height.
  $effect.pre(() => {
    if (open) z = 100 + ++opened;
  });

  $effect(() => {
    if (open) {
      restore = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      queueMicrotask(() => panel?.querySelector<HTMLElement>('button, [href], input, select, textarea, summary')?.focus());
    } else {
      restore?.focus();
      restore = null;
    }
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onclose();
    } else if (e.key === 'Tab' && panel) {
      // Keep focus inside the sheet while it is open.
      const f = [...panel.querySelectorAll<HTMLElement>('button, [href], input, select, textarea, summary')].filter((x) => !x.hasAttribute('disabled'));
      const first = f[0], last = f[f.length - 1];
      if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last?.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); }
    }
  }
</script>

{#if open}
  <div class="fixed inset-0 flex {right ? 'justify-end' : 'items-end justify-center sm:items-center'}" style="z-index:{z}" role="presentation" data-sheet {onkeydown}>
    <button class="absolute inset-0 bg-black/50" aria-label="Close" tabindex="-1" onclick={onclose} transition:fade={{ duration: motionMs(MOTION.fast) }}></button>
    <div
      bind:this={panel}
      role="dialog"
      aria-modal="true"
      aria-label={title}
      data-side={side}
      transition:fly={right ? { x: motionPx(640), duration: motionMs(MOTION.base + 40) } : { y: motionPx(40), duration: motionMs(MOTION.base) }}
      class="sheet-panel bg-solid-2 backdrop-blur {right
        ? 'h-full w-1/2 max-w-full min-w-[min(26rem,100vw)] rounded-l-xl border-r-0 px-4 pb-6'
        : 'max-h-[92dvh] w-full max-w-md rounded-t-xl p-4 pb-6 sm:rounded-xl'}"
    >
      {#if right}
        <div class="sticky top-0 z-10 -mx-4 mb-2 flex items-center justify-between border-b border-line bg-solid-2 px-6 py-3">
          <h2 class="t-subtitle">{title}</h2>
          <IconButton icon="close" label="Close {title}" onclick={onclose} />
        </div>
      {:else}
        <h2 class="group-title pb-2 pt-1">{title}</h2>
      {/if}
      {@render children()}
    </div>
  </div>
{/if}
