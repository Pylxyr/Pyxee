<script module lang="ts">
  // Sheets stack in the order they were opened, not in the order they appear in the page: a sheet
  // opened from another sheet (e.g. the theme editor from Settings) must land on top of it.
  // 100 and up keeps every sheet above Now Playing (z-40), the dock, the deck and the top bar;
  // the snackbar sits above all of them so its Undo stays reachable.
  let opened = 0;
</script>

<script lang="ts">
  import { fly, fade } from 'svelte/transition';
  import { cubicIn, cubicOut, expoOut } from 'svelte/easing';
  import type { Snippet } from 'svelte';
  import { MOTION, motionMs, motionPx } from '$lib/motion';
  import IconButton from './IconButton.svelte';
  import { focusables } from '$lib/focus';

  // `bottom` (the default) is a card that rises from the bottom, centred on wide windows. `left` and `right` are tabs
  // that slide in from that edge and take half of the window (all of it when the window is narrow); Settings uses
  // them, on the side its button is on.
  // `dialog` is a large centred panel that brings its own header and layout (Settings): it takes most of a wide window.
  let { open, title, onclose, children, side = 'bottom', returnFocus = null }: { open: boolean; title: string; onclose: () => void; children: Snippet; side?: 'bottom' | 'left' | 'right' | 'dialog'; returnFocus?: HTMLElement | null } = $props();
  const right = $derived(side === 'right');
  const dialog = $derived(side === 'dialog');
  const tab = $derived(side === 'left' || side === 'right');
  let panel = $state<HTMLElement>();
  let restore: HTMLElement | null = null;
  let z = $state(100);

  // Before the sheet is drawn, so it never appears for a frame at the wrong height.
  $effect.pre(() => {
    if (open) z = 100 + ++opened;
  });

  $effect(() => {
    if (open) {
      // Focus goes back to what opened the sheet: the button that had focus, or `returnFocus` when it was opened without
      // one (a right-click on a row leaves focus on the page, not on the row).
      const active = document.activeElement instanceof HTMLElement && document.activeElement !== document.body ? document.activeElement : null;
      restore = returnFocus?.isConnected ? returnFocus : active;
      queueMicrotask(() => (panel ? focusables(panel)[0] : undefined)?.focus());
    } else {
      if (restore?.isConnected) restore.focus({ preventScroll: true });
      restore = null;
    }
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onclose();
    } else if (e.key === 'Tab' && panel) {
      // Keep focus inside the sheet while it is open.
      const f = focusables(panel);
      const first = f[0], last = f[f.length - 1];
      // Focus that has got outside the sheet (to the page behind it) is brought back in.
      if (!panel.contains(document.activeElement) && f.length) { e.preventDefault(); (e.shiftKey ? last : first)?.focus(); }
      else if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last?.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); }
    }
  }
</script>

{#if open}
  <div class="fixed inset-0 flex {right ? 'justify-end' : tab ? 'justify-start' : dialog ? 'items-center justify-center' : 'items-end justify-center sm:items-center'}" style="z-index:{z}" role="presentation" data-sheet {onkeydown}>
    <button class="absolute inset-0 bg-black/50" aria-label="Close" tabindex="-1" onclick={onclose} in:fade={{ duration: motionMs(MOTION.fast) }} out:fade={{ duration: motionMs(MOTION.exit) }}></button>
    <div
      bind:this={panel}
      role="dialog"
      aria-modal="true"
      aria-label={title}
      data-side={side}
      in:fly={dialog ? { y: motionPx(24), duration: motionMs(MOTION.base + 40), easing: expoOut } : tab ? { x: motionPx(right ? 640 : -640), duration: motionMs(MOTION.base + 80), easing: expoOut } : { y: motionPx(40), duration: motionMs(MOTION.base), easing: cubicOut }}
      out:fly={dialog ? { y: motionPx(16), duration: motionMs(MOTION.exit), easing: cubicIn } : tab ? { x: motionPx(right ? 640 : -640), duration: motionMs(MOTION.exit + 30), easing: cubicIn } : { y: motionPx(40), duration: motionMs(MOTION.exit), easing: cubicIn }}
      class="sheet-panel bg-solid-2 {dialog
        ? 'sheet-dialog h-[min(48rem,94dvh)] w-[min(66rem,100vw)] max-w-full overflow-hidden rounded-xl p-0 max-sm:h-dvh max-sm:rounded-none'
        : right
        ? 'h-full w-1/2 max-w-full min-w-[min(26rem,100vw)] rounded-l-xl border-r-0 px-4 pb-6'
        : tab
          ? 'h-full w-1/2 max-w-full min-w-[min(26rem,100vw)] rounded-r-xl border-l-0 px-4 pb-6'
          : 'max-h-[92dvh] w-full max-w-md rounded-t-xl p-4 pb-6 sm:rounded-xl'}"
    >
      {#if dialog}
        <!-- the dialog draws its own header -->
      {:else if tab}
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
