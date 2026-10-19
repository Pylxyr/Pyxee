<script lang="ts">
  import { player } from '$lib/player.svelte';
  import { relatedStore } from '$lib/related.svelte';
  import Skeleton from './Skeleton.svelte';
  import TrackRow from './TrackRow.svelte';

  let { active }: { active: boolean } = $props();

  const track = $derived(player.shown?.track ?? null);
  // Looked up when the tab is showing (and again, after a short wait, when the song changes while it is).
  $effect(() => {
    if (active) relatedStore.load(track);
  });

  const st = $derived(relatedStore.state);
  const mine = $derived(track != null && st.kind !== 'idle' && st.key === track.key);
  const tracks = $derived(mine && st.kind === 'ready' ? st.tracks : []);
</script>

<div class="max-h-full overflow-y-auto px-1 py-2" data-testid="related" role="region" aria-label="Related songs">
  {#if !track}
    <p class="px-3 py-6 text-sm text-muted">Play a song to see similar music.</p>
  {:else if !mine || st.kind === 'loading'}
    <div role="status" aria-label="Looking for related songs"><Skeleton rows={5} /></div>
  {:else if st.kind === 'error'}
    <p class="mx-2 rounded-md border border-line bg-surface px-4 py-3 text-sm" role="status">
      Couldn't load related songs. {st.error.message}
      <button class="ml-2 font-semibold underline" onclick={() => relatedStore.retry(track)}>Try again</button>
    </p>
  {:else if tracks.length === 0}
    <p class="px-3 py-6 text-sm text-muted">No related songs found for this one.</p>
  {:else}
    <div class="flex items-center justify-between gap-3 px-3 pb-2">
      <h2 class="text-[13px] font-semibold uppercase tracking-wider text-muted">Similar to this song</h2>
      <button
        class="shrink-0 rounded-md bg-accent px-3 py-1.5 text-sm font-semibold text-on-accent"
        aria-label="Play all related songs"
        onclick={() => player.playTracks(tracks, 0)}>Play all</button>
    </div>
    <ul aria-label="Related songs">
      {#each tracks as t, i (t.key)}
        <TrackRow track={t} index={i} onplay={() => player.playTracks(tracks, i)} />
      {/each}
    </ul>
  {/if}
</div>
