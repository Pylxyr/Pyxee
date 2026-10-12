<script lang="ts">
  import { hueFor } from '$lib/format';
  import { thumbCandidates } from '$lib/thumb';
  import Icon from './Icon.svelte';

  let { src, seed, size = 56, rounded = 'rounded-md', fill = false }: { src: string | null; seed: string; size?: number; rounded?: string; fill?: boolean } = $props();
  // The sharpest image for this box first; a 404 (e.g. no maxresdefault) moves on to the next.
  let tried = $state(0);
  const dpr = typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1;
  const candidates = $derived(src ? thumbCandidates(src, size, dpr) : []);
  const current = $derived(candidates[tried] ?? null);
  // A different picture (another song in a reused row) starts the search for a working URL again.
  $effect(() => {
    src;
    tried = 0;
  });
</script>

{#if current}
  <img
    src={current}
    alt=""
    loading="lazy"
    decoding="async"
    class="shrink-0 object-cover {rounded} {fill ? 'h-full w-full' : ''}"
    style={fill ? undefined : `width:${size}px;height:${size}px`}
    onerror={() => tried++}
  />
{:else}
  <div
    class="flex shrink-0 items-center justify-center text-white/80 {rounded} {fill ? 'h-full w-full' : ''}"
    style="{fill ? '' : `width:${size}px;height:${size}px;`}background:hsl({hueFor(seed)} 38% 32%)"
    aria-hidden="true"
  >
    <Icon name="music" size={Math.round(size / 2.6)} />
  </div>
{/if}
