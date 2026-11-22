<script lang="ts">
  import { availability } from '$lib/availability.svelte';
  import { library } from '$lib/library.svelte';
  import { openTrackMenu } from '$lib/menu';
  import { player } from '$lib/player.svelte';
  import { artistOf, formatTime, titleOf } from '$lib/format';
  import { popOnChange, reveal } from '$lib/motion';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import Icon from './Icon.svelte';
  import IconButton from './IconButton.svelte';
  import PlayingMark from './PlayingMark.svelte';
  import Thumb from './Thumb.svelte';

  const MISSING_HINT = 'This file is missing: moved, renamed, deleted, or its drive is not connected.';

  /** `index`: the row's place in its list (0-based); it shows as a number that turns into a play mark on hover. */
  let { track, onplay, index }: { track: TrackDto; onplay: () => void; index?: number } = $props();
  const liked = $derived(library.isLiked(track.key));
  // A song on this PC whose file is gone says so, before it is played and skipped.
  const missing = $derived(availability.isMissing(track.key));
  $effect(() => {
    void availability.epoch;
    availability.want(track.key);
  });
  // The song that is playing is marked in every list it appears in, not only in the queue.
  const current = $derived(player.shown?.track.key === track.key);

  // Right-click (or the menu key) anywhere on the row opens the same menu as its "more" button.
  function openMenu(e: MouseEvent) {
    // A right-click leaves focus where it was: the menu gives it back to this row when it closes.
    openTrackMenu(track, { event: e, opener: (e.currentTarget as HTMLElement).querySelector<HTMLElement>('.trow-main') });
  }
</script>

<!-- A table row: place, small cover, title, artist (its own column when the list is wide), length. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<li class="trow" data-current={current} data-missing={missing} oncontextmenu={openMenu} use:reveal>
  <button class="trow-main" onclick={onplay} title={missing ? MISSING_HINT : undefined}>
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
      <span class="trow-title" dir="auto">{titleOf(track)}</span>
      <span class="trow-artist" dir="auto">{missing ? `File missing · ${artistOf(track)}` : artistOf(track)}</span>
    </span>
    <span class="trow-time">{formatTime(track.durationMs)}</span>
  </button>
  <span class="trow-actions" data-liked={liked} use:popOnChange={liked}>
    <IconButton icon="heart" iconSize={18} fill={liked} pressed={liked} label={liked ? `Unlike ${titleOf(track)}` : `Like ${titleOf(track)}`} onclick={() => library.toggleLike(track)} />
  </span>
  <span class="trow-actions">
    <IconButton icon="more" iconSize={18} label="More options for {titleOf(track)}" onclick={(e) => openTrackMenu(track, { button: e.currentTarget })} />
  </span>
</li>
