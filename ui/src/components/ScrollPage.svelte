<script lang="ts">
  import type { Snippet } from 'svelte';

  /** A page whose header stays put while only the body scrolls. */
  let { header, children }: { header: Snippet; children: Snippet } = $props();
</script>

<section class="flex h-full min-h-0 flex-col">
  <div class="page-pad shrink-0" data-testid="page-header">{@render header()}</div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="page-pad min-h-0 flex-1 overflow-y-auto pb-8" data-testid="page-scroll" data-scroll tabindex="0">{@render children()}</div>
</section>
