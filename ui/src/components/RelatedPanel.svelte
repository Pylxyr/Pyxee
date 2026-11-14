<script lang="ts">
  import { player } from '$lib/player.svelte';
  import { relatedStore } from '$lib/related.svelte';
  import Skeleton from './Skeleton.svelte';
  import Button from './Button.svelte';
  import TrackList from './TrackList.svelte';

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
    <p class="muted px-3 py-6">Play a song to see similar music.</p>
  {:else if !mine || st.kind === 'loading'}
    <div role="status" aria-label="Looking for related songs"><Skeleton rows={5} /></div>
  {:else if st.kind === 'error'}
    <p class="notice mx-2" role="status">
      Couldn't load related songs. {st.error.message}
      <button class="btn-link ml-2" onclick={() => relatedStore.retry(track)}>Try again</button>
    </p>
  {:else if tracks.length === 0}
    <p class="muted px-3 py-6">No related songs found for this one.</p>
  {:else}
    <div class="flex items-center justify-between gap-3 px-3 pb-2">
      <h2 class="t-strong">Similar to this song</h2>
      <Button variant="primary" size="sm" aria-label="Play all related songs" onclick={() => player.playTracks(tracks, 0)}>Play all</Button>
    </div>
    <TrackList label="Related songs" head={false} {tracks} onplay={(i) => player.playTracks(tracks, i)} />
  {/if}
</div>
