<script lang="ts">
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import { openTrackMenu } from '$lib/menu';
  import type { MenuExtra } from '$lib/nav.svelte';
  import { artistOf, titleOf } from '$lib/format';
  import { reveal } from '$lib/motion';
  import Icon from './Icon.svelte';
  import Thumb from './Thumb.svelte';

  /** Songs as covers (Explore's shelves): for browsing by sight instead of by name. With `rows` they are compact rows (a small cover beside the title and artist), several to a line, that do not scroll sideways (Home's Recently played). */
  let { tracks, label, onplay, extras, rows = false }: { tracks: TrackDto[]; label: string; onplay: (index: number) => void; extras?: (t: TrackDto) => MenuExtra[]; rows?: boolean } = $props();

  // A right-click (or the Menu key) opens the same song menu as on a queue row; the sheet gives focus back to the tile.
  function openMenu(e: MouseEvent, t: TrackDto) {
    openTrackMenu(t, { event: e, opener: (e.currentTarget as HTMLElement).querySelector<HTMLElement>('.cover-tile'), extras: extras?.(t) });
  }
</script>

<ul class="cover-grid" class:rows aria-label={label}>
  {#each tracks as t, i (t.key)}
    <li oncontextmenu={(e) => openMenu(e, t)} use:reveal>
      <button class="cover-tile" aria-label="Play {titleOf(t)}" onclick={() => onplay(i)}>
        <span class="cover-art block">
          <Thumb src={t.thumbnail} seed={t.key} size={160} fill rounded="r-md" />
          <span class="cover-play" aria-hidden="true"><Icon name="play" size={18} fill /></span>
        </span>
        <span class="trow-title mt-2 block" dir="auto">{titleOf(t)}</span>
        <span class="trow-artist block" dir="auto">{artistOf(t)}</span>
      </button>
    </li>
  {/each}
</ul>
