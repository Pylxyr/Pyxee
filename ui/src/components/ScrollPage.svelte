<script lang="ts">
  import type { Snippet } from 'svelte';

  /** A page whose header stays put while only the body scrolls. */
  let { header, children }: { header: Snippet; children: Snippet } = $props();

  // Once the body has scrolled, a soft edge appears under the pinned header (content now passes beneath it). One state
  // flip when the scroll crosses a few pixels, not per scroll event; the edge is a pre-rendered gradient layer whose
  // opacity changes (see `.page-edge` in app.css). Event-driven on purpose: a scroll-linked animation would count as a
  // running animation while a song plays.
  const EDGE_AT = 8;
  let scrolled = $state(false);
  const onscroll = (e: Event) => {
    const past = (e.currentTarget as HTMLElement).scrollTop > EDGE_AT;
    if (past !== scrolled) scrolled = past;
  };
</script>

<section class="flex h-full min-h-0 flex-col" data-scrolled={scrolled}>
  <div class="page-pad page-edge shrink-0" data-testid="page-header">{@render header()}</div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="page-pad min-h-0 flex-1 overflow-y-auto pb-8" data-testid="page-scroll" data-scroll tabindex="0" {onscroll}>{@render children()}</div>
</section>
