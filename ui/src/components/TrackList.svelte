<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import { withKeys } from '$lib/keys';
  import TrackRow from './TrackRow.svelte';

  /**
   * A list of songs as a table. Give it `tracks` and `onplay` for the usual rows, or put your own rows in as children
   * (Search mixes songs with artists and playlists). `head` draws the column names above the first row.
   */
  let { label, tracks, onplay, numbered = true, head = true, children }: {
    label: string;
    tracks?: TrackDto[];
    onplay?: (index: number) => void;
    numbered?: boolean;
    head?: boolean;
    children?: Snippet;
  } = $props();
  // Keyed by the song (and its occurrence), not by its place: a row keeps its cover when the rows before it change.
  const rows = $derived(tracks ? withKeys(tracks, (t) => t.key) : []);
</script>

<div class="tracklist">
  {#if head}
    <div class="tracklist-head" aria-hidden="true">
      <span>#</span><span>Title</span><span class="col-artist">Artist</span><span class="col-time">Time</span>
    </div>
  {/if}
  <ul aria-label={label}>
    {#if tracks}
      {#each rows as r (r.key)}
        <TrackRow track={r.item} index={numbered ? r.index : undefined} onplay={() => onplay?.(r.index)} />
      {/each}
    {/if}
    {@render children?.()}
  </ul>
</div>
