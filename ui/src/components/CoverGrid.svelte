<script lang="ts">
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import Icon from './Icon.svelte';
  import Thumb from './Thumb.svelte';

  /** Songs as covers (Recently played, Jump back in): for browsing by sight instead of by name. */
  let { tracks, label, onplay }: { tracks: TrackDto[]; label: string; onplay: (index: number) => void } = $props();
</script>

<ul class="cover-grid" aria-label={label}>
  {#each tracks as t, i (t.key)}
    <li>
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
