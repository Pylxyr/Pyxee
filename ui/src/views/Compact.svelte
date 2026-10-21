<script lang="ts">
  // Shared by the tray flyout and the mini player: cover, title, artist, a thin progress line and the transport.
  import { player } from '$lib/player.svelte';
  import IconButton from '../components/IconButton.svelte';
  import PlayButton from '../components/PlayButton.svelte';
  import Thumb from '../components/Thumb.svelte';

  let {
    cover = 88,
    onopen,
  }: {
    cover?: number;
    /** Clicking the cover or the title (open the main window). */
    onopen?: () => void;
  } = $props();

  const entry = $derived(player.shown);
  const percent = $derived(
    player.durationMs ? Math.min(100, (player.state.positionMs / player.durationMs) * 100) : 0,
  );
</script>

<div class="compact">
  <button class="shrink-0 overflow-hidden r-md" style="width:{cover}px;height:{cover}px" aria-label="Open Pyxee" onclick={onopen} data-testid="compact-art">
    <Thumb src={entry?.track.thumbnail ?? null} seed={entry?.track.key ?? 'pyxee'} size={cover} fill rounded="r-md" />
  </button>
  <div class="flex min-w-0 flex-1 flex-col gap-1">
    <button class="min-w-0 text-left" onclick={onopen} tabindex="-1">
      <p class="t-strong truncate" data-testid="compact-title">{entry?.track.title ?? 'Nothing playing'}</p>
      <p class="t-caption muted truncate">{entry?.track.artist ?? ''}</p>
    </button>
    <div class="compact-bar" role="progressbar" aria-label="Progress" aria-valuenow={Math.round(percent)} aria-valuemin="0" aria-valuemax="100">
      <div style="width:{percent}%"></div>
    </div>
    <div class="flex items-center gap-2">
      <IconButton icon="prev" label="Previous" strong onclick={player.previous} />
      <PlayButton size="sm" announceLoading={false} />
      <IconButton icon="next" label="Next" strong onclick={player.next} />
    </div>
  </div>
</div>
