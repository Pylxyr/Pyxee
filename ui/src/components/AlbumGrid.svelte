<script lang="ts">
  import type { AlbumDto } from '$lib/bindings/AlbumDto';
  import Icon from './Icon.svelte';
  import Thumb from './Thumb.svelte';

  /**
   * Albums and singles as covers (Explore's "New albums & singles"). Pyxee has no album page, so a cover starts a search
   * for the release instead of opening it.
   */
  let { albums, label, onopen }: { albums: AlbumDto[]; label: string; onopen: (album: AlbumDto) => void } = $props();
</script>

<ul class="cover-grid" aria-label={label}>
  {#each albums as a (a.id)}
    <li>
      <button class="cover-tile" aria-label="Search for {a.title} by {a.artist}" onclick={() => onopen(a)}>
        <span class="cover-art block">
          <Thumb src={a.thumbnail} seed={a.id} size={160} fill rounded="r-md" />
          <span class="cover-play" aria-hidden="true"><Icon name="search" size={18} /></span>
        </span>
        <span class="trow-title mt-2 block">{a.title}</span>
        <span class="trow-artist block">{a.artist}</span>
      </button>
    </li>
  {/each}
</ul>
