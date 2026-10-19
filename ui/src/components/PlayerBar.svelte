<script lang="ts">
  import { player, describeFailure } from '$lib/player.svelte';
  import { prefs } from '$lib/prefs.svelte';
  import { nav } from '$lib/nav.svelte';
  import { library } from '$lib/library.svelte';
  import { formatTime } from '$lib/format';
  import Icon from './Icon.svelte';
  import Thumb from './Thumb.svelte';
  import Seek from './Seek.svelte';

  let { wide }: { wide: boolean } = $props();
  const entry = $derived(player.shown);
  const liked = $derived(entry ? library.isLiked(entry.track.key) : false);
  const playLabel = $derived(player.isPlaying ? 'Pause' : player.isBusy ? 'Loading, press to pause' : 'Play');
</script>

{#if entry}
  <div class="relative shrink-0 border-t border-line bg-surface {player.isPlaying ? 'shadow-[inset_0_2px_0_0_var(--accent)]' : ''}" data-testid="player-bar">
    {#if !wide}
      <div class="absolute inset-x-0 -top-2 z-10">
        <Seek positionMs={player.state.positionMs} durationMs={player.durationMs} onseek={player.seek} accent label="Seek" />
      </div>
    {/if}
    <div class="flex h-[76px] items-center gap-4 px-4 sm:px-6">
      <button class="flex min-w-0 flex-1 items-center gap-3 text-left" aria-label="Open Now Playing" onclick={() => nav.openNowPlaying()}>
        <Thumb src={entry.track.thumbnail} seed={entry.track.key} size={48} rounded="rounded-sm" />
        <span class="min-w-0">
          <span class="label block !text-[10px] {player.isPlaying ? '!text-accent' : ''}" aria-hidden="true">{player.failure ? 'Stopped' : player.isPlaying ? 'Playing' : player.isBusy ? 'Loading' : 'Paused'}</span>
          <span class="block truncate text-[15px] font-semibold">{entry.track.title}</span>
          {#if player.failure}
            <span class="block truncate text-[13px] text-accent" role="alert">{describeFailure(player.failure)}</span>
          {:else}
            <span class="block truncate text-[13px] text-muted">{entry.track.artist}</span>
          {/if}
        </span>
      </button>

      {#if player.failure}
        <button class="rounded-md bg-accent px-4 py-1.5 text-sm font-semibold text-on-accent" onclick={player.retry}>Retry</button>
      {/if}

      <div class="flex items-center gap-3 sm:gap-5">
        {#if wide}
          <button class="p-1" aria-label="Previous" onclick={player.previous}><Icon name="prev" /></button>
        {/if}
        <button class="flex h-11 w-11 items-center justify-center rounded bg-accent text-on-accent" aria-label={playLabel} onclick={player.toggle}>
          {#if player.state.transport.state === 'loading'}
            <span class="h-5 w-5 animate-spin rounded-full border-2 border-on-accent/30 border-t-on-accent" aria-hidden="true"></span>
          {:else}
            <Icon name={player.isPlaying ? 'pause' : 'play'} fill={!player.isPlaying} />
          {/if}
        </button>
        <button class="p-1" aria-label="Next" onclick={player.next}><Icon name="next" /></button>
        {#if wide}
          <div class="ml-2 flex w-72 items-center gap-2 text-[12px] tabular-nums text-muted">
            <span class="w-10 text-right">{formatTime(player.state.positionMs)}</span>
            <div class="min-w-0 flex-1"><Seek positionMs={player.state.positionMs} durationMs={player.durationMs} onseek={player.seek} accent label="Seek" /></div>
            <span class="w-10">{player.durationMs ? formatTime(player.durationMs) : '--:--'}</span>
          </div>
        {/if}
      </div>

      {#if wide}
        <div class="flex flex-1 items-center justify-end gap-3">
          <Icon name="volume" size={20} />
          <div class="w-28">
            <input
              type="range"
              class="slider"
              min="0"
              max="100"
              value={prefs.volume}
              aria-label="Volume"
              style="--pct:{prefs.volume}%"
              oninput={(e) => prefs.setVolume(e.currentTarget.valueAsNumber)}
            />
          </div>
          <button class="p-1 {liked ? 'text-accent' : 'text-muted hover:text-fg'}" aria-label={liked ? 'Unlike' : 'Like'} aria-pressed={liked} onclick={() => entry && library.toggleLike(entry.track)}><Icon name="heart" fill={liked} /></button>
          <button class="p-1" aria-label="Open queue" onclick={() => nav.go('queue')}><Icon name="queue" /></button>
          <button class="p-1" aria-label="Expand Now Playing" onclick={() => nav.openNowPlaying()}><Icon name="up" /></button>
        </div>
      {/if}
    </div>
  </div>
{/if}
