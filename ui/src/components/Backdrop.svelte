<script lang="ts">
  import { hueFor } from '$lib/format';
  import { BACKDROP_PX, backdropCandidates, paintBackdrop } from '$lib/backdrop';

  let { src, seed, class: cls = '' }: { src: string | null; seed: string; class?: string } = $props();
  let canvas = $state<HTMLCanvasElement | null>(null);

  // Draws once per cover. What was shown stays until the new picture has loaded, so changing song never flashes.
  $effect(() => {
    const c = canvas;
    if (!c) return;
    const urls = backdropCandidates(src);
    const hue = hueFor(seed);
    let stale = false;
    if (urls.length === 0) paintBackdrop(c, null, hue);
    const attempt = (i: number) => {
      if (stale || i >= urls.length) return;
      const img = new Image();
      img.decoding = 'async';
      img.onload = () => {
        if (!stale) paintBackdrop(c, img, hue);
      };
      img.onerror = () => attempt(i + 1);
      img.src = urls[i]!;
    };
    attempt(0);
    return () => {
      stale = true;
    };
  });
</script>

<canvas bind:this={canvas} width={BACKDROP_PX} height={BACKDROP_PX} class="backdrop {cls}" aria-hidden="true"></canvas>
