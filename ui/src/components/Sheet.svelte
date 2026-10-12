<script module lang="ts">
  // Sheets stack in the order they were opened, not in the order they appear in the page: a sheet
  // opened from another sheet (e.g. the theme editor from Settings) must land on top of it.
  // 100 and up keeps every sheet above Now Playing (z-40), the player bar and the top bar;
  // the snackbar sits above all of them so its Undo stays reachable.
  let opened = 0;
</script>

<script lang="ts">
  import { fly, fade } from 'svelte/transition';
  import type { Snippet } from 'svelte';

  let { open, title, onclose, children }: { open: boolean; title: string; onclose: () => void; children: Snippet } = $props();
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
  <div class="fixed inset-0 flex items-end justify-center sm:items-center" style="z-index:{z}" role="presentation" data-sheet {onkeydown}>
    <button class="absolute inset-0 bg-black/50" aria-label="Close" tabindex="-1" onclick={onclose} transition:fade={{ duration: 120 }}></button>
    <div
      bind:this={panel}
      role="dialog"
      aria-modal="true"
      aria-label={title}
      transition:fly={{ y: 40, duration: 160 }}
      class="relative max-h-[92dvh] w-full max-w-md overflow-y-auto rounded-t-2xl bg-solid-2 p-4 pb-6 shadow-xl backdrop-blur sm:rounded-2xl"
    >
      <h2 class="px-2 pb-2 text-sm font-semibold text-muted">{title}</h2>
      {@render children()}
    </div>
  </div>
{/if}
