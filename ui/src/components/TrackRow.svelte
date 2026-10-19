<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { formatTime } from '$lib/format';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import Icon from './Icon.svelte';
  import Thumb from './Thumb.svelte';

  /** `index`: the row's place in its list (0-based); it shows as a number that turns into a play mark on hover. */
  let { track, onplay, index }: { track: TrackDto; onplay: () => void; index?: number } = $props();
  const liked = $derived(library.isLiked(track.key));
</script>

<!-- A table row: place, small cover, title, artist (its own column when there is room), length. -->
<li class="group flex items-center border-b border-line/60 hover:bg-surface-2/70">
  <button class="flex min-w-0 flex-1 items-center gap-3 py-1.5 pl-2 pr-1 text-left" onclick={onplay}>
    <span class="flex w-6 shrink-0 items-center justify-center text-[12px] tabular-nums text-muted" aria-hidden="true">
      {#if index !== undefined}<span class="group-hover:hidden">{index + 1}</span><span class="hidden text-fg group-hover:inline"><Icon name="play" size={14} fill /></span>{:else}<span class="hidden text-fg group-hover:inline"><Icon name="play" size={14} fill /></span>{/if}
    </span>
    <Thumb src={track.thumbnail} seed={track.key} size={40} rounded="rounded-sm" />
    <span class="flex min-w-0 flex-1 flex-col md:flex-row md:items-center md:gap-4">
      <span class="block truncate text-[14px] font-medium md:flex-1">{track.title}</span>
      <span class="block truncate text-[12px] text-muted md:w-[32%] md:shrink-0 md:text-[13px]">{track.artist}</span>
    </span>
    <span class="w-10 shrink-0 text-right text-[12px] tabular-nums text-muted">{formatTime(track.durationMs)}</span>
  </button>
  <button
    class="p-2 {liked ? 'text-accent' : 'text-muted opacity-60 hover:text-fg group-hover:opacity-100'}"
    aria-label={liked ? `Unlike ${track.title}` : `Like ${track.title}`}
    aria-pressed={liked}
    onclick={() => library.toggleLike(track)}><Icon name="heart" size={18} fill={liked} /></button>
  <button class="p-2 text-muted opacity-60 hover:text-fg group-hover:opacity-100" aria-label="More options for {track.title}" onclick={() => (nav.menuTrack = track)}><Icon name="more" size={18} /></button>
</li>
