<script lang="ts">
  // Shared by the tray flyout and the mini player: cover, title, artist, a thin progress line and the transport.
  import { player } from '$lib/player.svelte';
  import Icon from '../components/Icon.svelte';
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

<div class="flex min-w-0 flex-1 items-center gap-3">
  <button class="shrink-0 overflow-hidden rounded-lg" style="width:{cover}px;height:{cover}px" aria-label="Open Pyxee" onclick={onopen} data-testid="compact-art">
    <Thumb src={entry?.track.thumbnail ?? null} seed={entry?.track.key ?? 'pyxee'} size={cover} fill rounded="rounded-lg" />
  </button>
  <div class="flex min-w-0 flex-1 flex-col gap-1.5">
    <button class="min-w-0 text-left" onclick={onopen} tabindex="-1">
      <p class="truncate text-[15px] font-semibold" data-testid="compact-title">{entry?.track.title ?? 'Nothing playing'}</p>
      <p class="truncate text-[13px] text-muted">{entry?.track.artist ?? ''}</p>
    </button>
    <div class="h-[3px] w-full overflow-hidden rounded-full bg-surface-2" role="progressbar" aria-label="Progress" aria-valuenow={Math.round(percent)} aria-valuemin="0" aria-valuemax="100">
      <div class="h-full rounded-full bg-accent" style="width:{percent}%"></div>
    </div>
    <div class="flex items-center gap-4">
      <button class="p-1 text-muted hover:text-fg" aria-label="Previous" onclick={player.previous}><Icon name="prev" size={22} /></button>
      <button class="flex h-9 w-9 items-center justify-center rounded-full bg-fg text-on-fg" aria-label={player.isPlaying ? 'Pause' : 'Play'} onclick={player.toggle}>
        <Icon name={player.isPlaying ? 'pause' : 'play'} size={20} fill={!player.isPlaying} />
      </button>
      <button class="p-1 text-muted hover:text-fg" aria-label="Next" onclick={player.next}><Icon name="next" size={22} /></button>
    </div>
  </div>
</div>
