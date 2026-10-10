<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { formatTime } from '$lib/format';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import Icon from './Icon.svelte';
  import Thumb from './Thumb.svelte';

  let { track, onplay }: { track: TrackDto; onplay: () => void } = $props();
  const liked = $derived(library.isLiked(track.key));
</script>

<li class="flex items-center rounded-lg hover:bg-surface">
  <button class="flex min-w-0 flex-1 items-center gap-3 p-2 text-left" onclick={onplay}>
    <Thumb src={track.thumbnail} seed={track.key} size={56} />
    <span class="min-w-0 flex-1">
      <span class="block truncate text-[15px] font-semibold">{track.title}</span>
      <span class="block truncate text-[13px] text-muted">{track.artist}</span>
    </span>
    <span class="text-[13px] tabular-nums text-muted">{formatTime(track.durationMs)}</span>
  </button>
  <button
    class="p-2 {liked ? 'text-accent' : 'text-muted hover:text-fg'}"
    aria-label={liked ? `Unlike ${track.title}` : `Like ${track.title}`}
    aria-pressed={liked}
    onclick={() => library.toggleLike(track)}><Icon name="heart" size={20} fill={liked} /></button>
  <button class="p-2 text-muted hover:text-fg" aria-label="More options for {track.title}" onclick={() => (nav.menuTrack = track)}><Icon name="more" /></button>
</li>
