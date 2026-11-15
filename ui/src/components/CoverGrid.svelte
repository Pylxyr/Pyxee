<script lang="ts">
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import { nav } from '$lib/nav.svelte';
  import Icon from './Icon.svelte';
  import Thumb from './Thumb.svelte';

  /** Songs as covers (Recently played, Jump back in): for browsing by sight instead of by name. */
  let { tracks, label, onplay }: { tracks: TrackDto[]; label: string; onplay: (index: number) => void } = $props();

  // A right-click (or the Menu key) opens the same song menu as on a queue row; the sheet gives focus back to the tile.
  function openMenu(e: MouseEvent, t: TrackDto) {
    e.preventDefault();
    nav.menuOpener = (e.currentTarget as HTMLElement).querySelector<HTMLElement>('.cover-tile');
    nav.menuTrack = t;
  }
</script>

<ul class="cover-grid" aria-label={label}>
  {#each tracks as t, i (t.key)}
    <li oncontextmenu={(e) => openMenu(e, t)}>
      <button class="cover-tile" aria-label="Play {t.title}" onclick={() => onplay(i)}>
        <span class="cover-art block">
          <Thumb src={t.thumbnail} seed={t.key} size={160} fill rounded="r-md" />
          <span class="cover-play" aria-hidden="true"><Icon name="play" size={18} fill /></span>
        </span>
        <span class="trow-title mt-2 block">{t.title}</span>
        <span class="trow-artist block">{t.artist}</span>
      </button>
    </li>
  {/each}
</ul>
