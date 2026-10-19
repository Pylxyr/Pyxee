<script lang="ts">
  import type { Snippet } from 'svelte';

  /** A page section's heading: small caps, an optional count, and a hairline that carries the eye across. */
  let { level = 2, count = null, children, actions }: { level?: 1 | 2; count?: number | string | null; children: Snippet; actions?: Snippet } = $props();
</script>

<div class="mb-1 mt-8 flex items-center gap-3 first:mt-0">
  <svelte:element this={level === 1 ? 'h1' : 'h2'} class="label min-w-0 truncate">{@render children()}</svelte:element>
  {#if count !== null}<span class="text-[11px] tabular-nums text-muted">{count}</span>{/if}
  <span class="h-px min-w-4 flex-1 bg-line" aria-hidden="true"></span>
  {@render actions?.()}
</div>
