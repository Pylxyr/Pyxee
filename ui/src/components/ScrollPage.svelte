<script lang="ts">
  import type { Snippet } from 'svelte';

  /** A page whose header stays put while only the body scrolls. */
  let { header, children }: { header: Snippet; children: Snippet } = $props();
</script>

<section class="flex h-full min-h-0 flex-col">
  <div class="shrink-0 px-4 pb-3 sm:px-8" data-testid="page-header">{@render header()}</div>
  <div class="min-h-0 flex-1 overflow-y-auto px-4 pb-8 sm:px-8" data-testid="page-scroll">{@render children()}</div>
</section>
