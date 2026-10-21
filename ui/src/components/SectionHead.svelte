<script lang="ts">
  import type { Snippet } from 'svelte';

  /** A heading inside a page: sentence case, an optional count and the section's own actions on the right. */
  let { level = 2, count = null, children, actions }: { level?: 1 | 2; count?: number | string | null; children: Snippet; actions?: Snippet } = $props();
</script>

<div class="section-head">
  <svelte:element this={level === 1 ? 'h1' : 'h2'} class="t-lead min-w-0 truncate">{@render children()}</svelte:element>
  {#if count !== null}<span class="t-caption muted nums">{count}</span>{/if}
  <span class="flex-1"></span>
  {@render actions?.()}
</div>
