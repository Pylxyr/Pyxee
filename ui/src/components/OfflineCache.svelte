<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { messageOf } from '$lib/backend';
  import type { AudioCacheDto } from '$lib/bindings/AudioCacheDto';
  import Button from './Button.svelte';

  const size = (mb: number) => (mb >= 1024 ? `${+(mb / 1024).toFixed(2)} GB` : `${mb} MB`);

  // Off, then 64, 128 and 256 MB, then every 256 MB up to 4 GB (the most the app allows).
  const STEPS = [0, 64, 128, ...Array.from({ length: 16 }, (_, i) => (i + 1) * 256)];
  const LIMITS: { mb: number; label: string }[] = STEPS.map((mb) => ({ mb, label: mb === 0 ? 'Off' : size(mb) }));

  let cache = $state<AudioCacheDto | null>(null);

  // Read when the page opens: what plays since then has been kept meanwhile.
  $effect(() => {
    const b = library.backend;
    if (!b) return;
    void b.audioCacheGet().then((c) => (cache = c)).catch((e) => snackbar.push(messageOf(e)));
  });

  const limits = $derived(cache && !LIMITS.some((l) => l.mb === cache!.limitMb) ? [...LIMITS, { mb: cache.limitMb, label: size(cache.limitMb) }] : LIMITS);

  async function setLimit(mb: number) {
    try {
      cache = await library.backend!.audioCacheSetLimit(mb);
    } catch (e) {
      snackbar.push(messageOf(e));
    }
  }

  async function clear() {
    try {
      cache = await library.backend!.audioCacheClear();
      snackbar.push('Kept songs removed.');
    } catch (e) {
      snackbar.push(messageOf(e));
    }
  }
</script>

<section class="flex max-w-xl flex-col gap-4" aria-label="Offline songs">
  <p class="muted">
    Songs you play are kept on this PC, so they play again at once, without the network and when YouTube does not answer. The songs you
    played longest ago are removed first when the limit is reached.
  </p>
  {#if cache}
    <label class="flex flex-col gap-1">
      <span>Keep up to</span>
      <select class="field" aria-label="Offline songs limit" value={cache.limitMb} onchange={(e) => setLimit(Number(e.currentTarget.value))}>
        {#each limits as l (l.mb)}
          <option value={l.mb}>{l.label}</option>
        {/each}
      </select>
    </label>
    <p role="status">
      {#if cache.limitMb === 0}
        Nothing is kept.
      {:else}
        {cache.songs} {cache.songs === 1 ? 'song' : 'songs'} kept, {size(cache.usedMb)} of {size(cache.limitMb)}.
      {/if}
    </p>
    <div><Button onclick={clear} disabled={cache.songs === 0}>Remove kept songs</Button></div>
  {/if}
</section>
