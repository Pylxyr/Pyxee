<script lang="ts">
  import { hueFor } from '$lib/format';
  import { network } from '$lib/network.svelte';
  import { isKnownMissing, isMissingStill, noteMissing, thumbCandidates } from '$lib/thumb';
  import { viewport } from '$lib/viewport.svelte';
  import Icon from './Icon.svelte';

  let { src, seed, size = 56, rounded = 'rounded-md', fill = false }: { src: string | null; seed: string; size?: number; rounded?: string; fill?: boolean } = $props();
  // The sharpest image for this box first; a 404 (e.g. no maxresdefault) moves on to the next.
  let tried = $state(0);
  // Bumped to create a fresh <img> (a new request) when a cover is still not showing after the window changed size.
  let attempt = $state(0);
  let loaded = $state(false);
  // True once a request failed (as opposed to the server saying "there is no picture"): only that is worth retrying.
  let transient = false;
  // Retries used since the last change of picture or return of the network.
  let retries = 0;
  // Wait before each retry; three tries, then it stays as it is. While offline nothing is scheduled at all.
  const RETRY_DELAYS = [2_000, 8_000, 30_000];
  viewport.watch();
  network.watch();
  const candidates = $derived(src ? thumbCandidates(src, size, viewport.dpr).filter((u) => !isKnownMissing(u)) : []);
  const current = $derived(candidates[tried] ?? null);
  // Every candidate has been tried and none gave a picture.
  const gaveUp = $derived(!!src && current === null);
  // A different picture (another song in a reused row) starts the search for a working URL again.
  $effect(() => {
    src;
    tried = 0;
    loaded = false;
    transient = false;
    retries = 0;
  });
  // A cover that could not be fetched is requested again, but only when it can work: never while the PC is offline
  // (the `online` event is the signal instead of polling), at most three times with growing waits, and never for a
  // cover the server said does not exist. A new `online` event gives a fresh set of tries.
  let seenNet = network.epoch;
  $effect(() => {
    const n = network.epoch;
    if (n !== seenNet) {
      seenNet = n;
      retries = 0;
    }
    if (!gaveUp || !transient || !network.online || retries >= RETRY_DELAYS.length) return;
    const timer = setTimeout(() => {
      retries++;
      tried = 0;
      attempt++;
    }, RETRY_DELAYS[retries]);
    return () => clearTimeout(timer);
  });
  function onloaded(e: Event) {
    const img = e.currentTarget as HTMLImageElement;
    // YouTube's grey "no picture" image arrives as a successful load: treat it as a miss and try the next URL.
    if (current && isMissingStill(current, img.naturalWidth, img.naturalHeight)) {
      noteMissing(current);
      tried++;
      return;
    }
    loaded = true;
  }
  function onfailed() {
    transient = true;
    tried++;
  }
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
    onload={onloaded}
    onerror={onfailed}
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
