<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { formatTime } from '$lib/format';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import Icon from './Icon.svelte';
  import IconButton from './IconButton.svelte';
  import PlayingMark from './PlayingMark.svelte';
  import Thumb from './Thumb.svelte';

  /** `index`: the row's place in its list (0-based); it shows as a number that turns into a play mark on hover. */
  let { track, onplay, index }: { track: TrackDto; onplay: () => void; index?: number } = $props();
  const liked = $derived(library.isLiked(track.key));
  // The song that is playing is marked in every list it appears in, not only in the queue.
  const current = $derived(player.shown?.track.key === track.key);
</script>

<!-- A table row: place, small cover, title, artist (its own column when the list is wide), length. -->
<li class="trow" data-current={current}>
  <button class="trow-main" onclick={onplay}>
    <span class="trow-idx" aria-hidden="true">
      {#if current}
        <PlayingMark playing={player.isPlaying} />
      {:else}
        {#if index !== undefined}<span class="when-rest">{index + 1}</span>{/if}
        <span class="when-hover"><Icon name="play" size={14} fill /></span>
      {/if}
    </span>
    <Thumb src={track.thumbnail} seed={track.key} size={40} rounded="r-sm" />
    <span class="trow-text">
      <span class="trow-title">{track.title}</span>
      <span class="trow-artist">{track.artist}</span>
    </span>
    <span class="trow-time">{formatTime(track.durationMs)}</span>
  </button>
  <span class="trow-actions" data-liked={liked}>
    <IconButton icon="heart" iconSize={18} fill={liked} pressed={liked} label={liked ? `Unlike ${track.title}` : `Like ${track.title}`} onclick={() => library.toggleLike(track)} />
  </span>
  <span class="trow-actions">
    <IconButton icon="more" iconSize={18} label="More options for {track.title}" onclick={() => (nav.menuTrack = track)} />
  </span>
</li>
