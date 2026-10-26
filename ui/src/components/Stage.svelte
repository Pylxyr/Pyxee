<script lang="ts">
  import { player, describeFailure } from '$lib/player.svelte';
  import { nav } from '$lib/nav.svelte';
  import { formatTime } from '$lib/format';
  import Button from './Button.svelte';
  import IconButton from './IconButton.svelte';
  import LikeButton from './LikeButton.svelte';
  import Logo from './Logo.svelte';
  import MenuItem from './MenuItem.svelte';
  import PlayButton from './PlayButton.svelte';
  import Seek from './Seek.svelte';
  import Thumb from './Thumb.svelte';
  import VolumeSlider from './VolumeSlider.svelte';

  // The player of a wide window, in two parts that the shell places on its grid: the deck along the bottom (what is
  // playing, the transport, and a seek line that runs the full width of the window), and from 1440 px the queue lane
  // beside the page, so what comes next stays in sight while you browse.
  let { lane = false }: { lane?: boolean } = $props();
  const entry = $derived(player.shown);

  // The bar's empty space opens the queue; the buttons and sliders in it keep their own jobs.
  function openQueue(e: MouseEvent) {
    if ((e.target as HTMLElement).closest('button, input, [role="slider"]')) return;
    nav.go('queue');
  }
</script>

{#if entry && lane}
  <aside class="lane" aria-label="Up next" data-testid="queue-lane">
    <div class="lane-head">
      <h2 class="t-lead">Up next</h2>
      <Button variant="quiet" size="sm" onclick={() => nav.go('queue')}>Open queue</Button>
    </div>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <ul class="lane-list" data-scroll tabindex="0">
      {#key entry.track.key}
      <li class="lane-now track-in">
        <Thumb src={entry.track.thumbnail} seed={entry.track.key} size={40} rounded="r-sm" />
        <span class="min-w-0 flex-1">
          <span class="trow-title accent-text block">{entry.track.title}</span>
          <span class="trow-artist block">Now playing</span>
        </span>
        <span class="trow-time">{formatTime(entry.track.durationMs)}</span>
      </li>
      {/key}
      {#each player.upNext as e (e.id)}
        <li>
          <MenuItem onclick={() => player.playEntry(e.id)}>
            <Thumb src={e.track.thumbnail} seed={e.track.key} size={40} rounded="r-sm" />
            <span class="min-w-0 flex-1">
              <span class="trow-title block">{e.track.title}</span>
              <span class="trow-artist block">{e.track.artist}</span>
            </span>
            <span class="trow-time">{formatTime(e.track.durationMs)}</span>
          </MenuItem>
        </li>
      {:else}
        <li class="t-caption muted px-2 py-3">Nothing queued after this song.</li>
      {/each}
    </ul>
  </aside>
{/if}

{#if entry}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <section class="deck" aria-label="Player" data-testid="player-bar" onclick={openQueue}>
    <div class="deck-seek">
      <Seek positionMs={player.state.positionMs} durationMs={player.durationMs} onseek={player.seek} accent label="Seek" />
    </div>
    {#key entry.track.key}
    <div class="deck-now track-in">
      <button class="deck-art" data-np-source aria-label="Open Now Playing" onclick={() => nav.openNowPlaying()}>
        <Thumb src={entry.track.thumbnail} seed={entry.track.key} size={72} fill rounded="r-sm" />
      </button>
      <div class="min-w-0 flex-1">
        <p class="t-strong truncate" title={entry.track.title}>{entry.track.title}</p>
        {#if player.failure}
          <p class="t-caption accent-text flex items-center gap-2" role="alert">
            <span class="min-w-0 flex-1 truncate">{describeFailure(player.failure)}</span>
            <Button variant="primary" size="sm" onclick={player.retry}>Retry</Button>
          </p>
        {:else}
          <p class="muted truncate">{entry.track.artist}</p>
        {/if}
      </div>
      <LikeButton track={entry.track} />
    </div>
    {/key}
    <div class="deck-transport">
      <IconButton icon="prev" label="Previous" size="lg" strong iconSize={24} onclick={player.previous} />
      <PlayButton />
      <IconButton icon="next" label="Next" size="lg" strong iconSize={24} onclick={player.next} />
      <span class="deck-time nums">{formatTime(player.state.positionMs)} / {player.durationMs ? formatTime(player.durationMs) : '--:--'}</span>
    </div>
    <div class="deck-extra">
      <div class="w-32"><VolumeSlider /></div>
      <IconButton icon="expand" label="Expand Now Playing" onclick={() => nav.openNowPlaying()} />
    </div>
  </section>
{:else}
  <section class="deck deck-empty" aria-label="Player">
    <Logo size={24} />
    <p class="muted">Nothing playing. Pick a song and it shows up here.</p>
  </section>
{/if}
