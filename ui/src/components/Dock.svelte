<script lang="ts">
  import { player, describeFailure } from '$lib/player.svelte';
  import { nav } from '$lib/nav.svelte';
  import Button from './Button.svelte';
  import IconButton from './IconButton.svelte';
  import PlayButton from './PlayButton.svelte';
  import Seek from './Seek.svelte';
  import Thumb from './Thumb.svelte';

  // The player of a narrow window: one strip above the bottom bar, with the seek line along its top edge.
  const entry = $derived(player.shown);
</script>

{#if entry}
  <div class="dock" data-testid="player-bar">
    <div class="dock-seek">
      <Seek positionMs={player.state.positionMs} durationMs={player.durationMs} onseek={player.seek} accent label="Seek" />
    </div>
    <button class="flex min-w-0 flex-1 items-center gap-3 text-left" aria-label="Open Now Playing" onclick={() => nav.openNowPlaying()}>
      <Thumb src={entry.track.thumbnail} seed={entry.track.key} size={44} rounded="r-sm" />
      <span class="min-w-0">
        <span class="t-strong block truncate">{entry.track.title}</span>
        {#if player.failure}
          <span class="t-caption block truncate accent-text" role="alert">{describeFailure(player.failure)}</span>
        {:else}
          <span class="t-caption muted block truncate">{entry.track.artist}</span>
        {/if}
      </span>
    </button>
    {#if player.failure}
      <Button variant="primary" size="sm" onclick={player.retry}>Retry</Button>
    {/if}
    <PlayButton />
    <IconButton icon="next" label="Next" size="lg" strong iconSize={24} onclick={player.next} />
  </div>
{/if}
