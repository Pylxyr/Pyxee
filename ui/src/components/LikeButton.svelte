<script lang="ts">
  import { library } from '$lib/library.svelte';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import IconButton from './IconButton.svelte';

  /** The heart for the song that is playing. (Rows in a list say which song in their own name; see TrackRow.) */
  let { track, size = 'md' }: { track: TrackDto; size?: 'md' | 'lg' } = $props();
  const liked = $derived(library.isLiked(track.key));
</script>

<IconButton icon="heart" fill={liked} pressed={liked} {size} label={liked ? 'Unlike' : 'Like'} onclick={() => library.toggleLike(track)} />
