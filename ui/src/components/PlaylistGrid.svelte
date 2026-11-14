<script lang="ts">
  import type { PlaylistCoverDto } from '$lib/bindings/PlaylistCoverDto';
  import Icon from './Icon.svelte';
  import Thumb from './Thumb.svelte';

  /** YouTube Music's own playlists as covers (a genre's picks): a cover plays the playlist. `busy` is the one being fetched. */
  let { playlists, label, busy = null, onplay }: { playlists: PlaylistCoverDto[]; label: string; busy?: string | null; onplay: (playlist: PlaylistCoverDto) => void } = $props();
</script>

<ul class="cover-grid" aria-label={label}>
  {#each playlists as p (p.id)}
    <li>
      <button class="cover-tile" aria-label="Play {p.title}" aria-busy={busy === p.id} disabled={busy !== null} onclick={() => onplay(p)}>
        <span class="cover-art block">
          <Thumb src={p.thumbnail} seed={p.id} size={160} fill rounded="r-md" />
          <span class="cover-play" aria-hidden="true"><Icon name="play" size={18} fill /></span>
        </span>
        <span class="trow-title mt-2 block">{p.title}</span>
        {#if p.subtitle}<span class="trow-artist block">{p.subtitle}</span>{/if}
      </button>
    </li>
  {/each}
</ul>
