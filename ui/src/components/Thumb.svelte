<script lang="ts">
  import { hueFor } from '$lib/format';
  import Icon from './Icon.svelte';

  let { src, seed, size = 56, rounded = 'rounded-md', fill = false }: { src: string | null; seed: string; size?: number; rounded?: string; fill?: boolean } = $props();
  let failed = $state(false);
</script>

{#if src && !failed}
  <img
    {src}
    alt=""
    loading="lazy"
    decoding="async"
    class="shrink-0 object-cover {rounded} {fill ? 'h-full w-full' : ''}"
    style={fill ? undefined : `width:${size}px;height:${size}px`}
    onerror={() => (failed = true)}
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
