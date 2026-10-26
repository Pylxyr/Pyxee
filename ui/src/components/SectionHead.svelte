<script lang="ts">
  import type { Snippet } from 'svelte';
  import IconButton from './IconButton.svelte';

  /**
   * A heading inside a page: sentence case, an optional count and the section's own actions on the right. When the
   * cover shelf under it scrolls sideways (inside `.shelves`), it also carries the buttons that page the shelf.
   */
  let { level = 2, count = null, children, actions }: { level?: 1 | 2; count?: number | string | null; children: Snippet; actions?: Snippet } = $props();

  let head = $state<HTMLElement>();
  let shelf = $state<HTMLElement | null>(null);
  let canBack = $state(false);
  let canForward = $state(false);

  function measure() {
    canBack = !!shelf && shelf.scrollLeft > 4;
    canForward = !!shelf && shelf.scrollLeft + shelf.clientWidth < shelf.scrollWidth - 4;
  }
  function page(direction: 1 | -1) {
    shelf?.scrollBy({ left: direction * shelf.clientWidth * 0.85, behavior: 'smooth' });
  }

  $effect(() => {
    const next = head?.nextElementSibling;
    shelf = next instanceof HTMLElement && next.classList.contains('cover-grid') && next.closest('.shelves') ? next : null;
    if (!shelf) return;
    measure();
    const el = shelf;
    el.addEventListener('scroll', measure, { passive: true });
    const watch = typeof ResizeObserver === 'function' ? new ResizeObserver(measure) : null;
    watch?.observe(el);
    return () => {
      el.removeEventListener('scroll', measure);
      watch?.disconnect();
    };
  });
</script>

<div class="section-head" bind:this={head}>
  <svelte:element this={level === 1 ? 'h1' : 'h2'} class="t-lead min-w-0 truncate">{@render children()}</svelte:element>
  {#if count !== null}<span class="t-caption muted nums section-count">{count}</span>{/if}
  <span class="flex-1"></span>
  {@render actions?.()}
  {#if shelf}
    <span class="shelf-nav">
      <IconButton icon="down" label="Scroll back" class="-rotate-90" disabled={!canBack} onclick={() => page(-1)} />
      <IconButton icon="down" label="Scroll forward" class="rotate-90" disabled={!canForward} onclick={() => page(1)} />
    </span>
  {/if}
</div>
