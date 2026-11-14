<script lang="ts">
  import { player } from '$lib/player.svelte';
  import Icon from './Icon.svelte';

  /**
   * The play / pause button, shared by the dock, the deck and Now Playing. `announceLoading` is what the dock and the
   * deck say while a song is still starting ("Loading, press to pause"); Now Playing keeps to Play and Pause.
   */
  let { size = 'md', announceLoading = true }: { size?: 'sm' | 'md' | 'lg'; announceLoading?: boolean } = $props();
  const label = $derived(player.isPlaying ? 'Pause' : announceLoading && player.isBusy ? 'Loading, press to pause' : 'Play');
</script>

<button class="play-btn {size === 'lg' ? 'play-btn-lg' : size === 'sm' ? 'play-btn-sm' : ''}" aria-label={label} onclick={player.toggle}>
  {#if player.state.transport.state === 'loading'}
    <span class="spinner" aria-hidden="true"></span>
  {:else}
    {#key player.isPlaying}
      <span class="pp-icon"><Icon name={player.isPlaying ? 'pause' : 'play'} size={size === 'lg' ? 28 : size === 'sm' ? 18 : 22} fill={!player.isPlaying} /></span>
    {/key}
  {/if}
</button>
