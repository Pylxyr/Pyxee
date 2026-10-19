<script lang="ts">
  import { hueFor } from '$lib/format';
  import { thumbCandidates } from '$lib/thumb';
  import { viewport } from '$lib/viewport.svelte';
  import Icon from './Icon.svelte';

  let { src, seed, size = 56, rounded = 'rounded-md', fill = false }: { src: string | null; seed: string; size?: number; rounded?: string; fill?: boolean } = $props();
  // The sharpest image for this box first; a 404 (e.g. no maxresdefault) moves on to the next.
  let tried = $state(0);
  // Bumped to create a fresh <img> (a new request) when a cover is still not showing after the window changed size.
  let attempt = $state(0);
  let loaded = $state(false);
  viewport.watch();
  const candidates = $derived(src ? thumbCandidates(src, size, viewport.dpr) : []);
  const current = $derived(candidates[tried] ?? null);
  // A different picture (another song in a reused row) starts the search for a working URL again.
  $effect(() => {
    src;
    tried = 0;
    loaded = false;
  });
  // After a fullscreen switch / resize / monitor change: a cover that never showed (blank, or every candidate failed
  // while the window was changing) is requested again from the sharpest candidate.
  let seenEpoch = viewport.epoch;
  $effect(() => {
    const e = viewport.epoch;
    if (e === seenEpoch) return;
    seenEpoch = e;
    if (!loaded) {
      tried = 0;
      attempt++;
    }
  });
</script>

{#if current}
  {#key attempt}
  <img
    src={current}
    alt=""
    loading="lazy"
    decoding="async"
    class="shrink-0 object-cover {rounded} {fill ? 'h-full w-full' : ''}"
    style={fill ? undefined : `width:${size}px;height:${size}px`}
    onload={() => (loaded = true)}
    onerror={() => tried++}
  />
  {/key}
{:else}
  <div
    class="flex shrink-0 items-center justify-center text-white/80 {rounded} {fill ? 'h-full w-full' : ''}"
    style="{fill ? '' : `width:${size}px;height:${size}px;`}background:hsl({hueFor(seed)} 38% 32%)"
    aria-hidden="true"
  >
    <Icon name="music" size={Math.round(size / 2.6)} />
  </div>
{/if}
