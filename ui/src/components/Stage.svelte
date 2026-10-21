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

  // The player of a wide window: a column of its own on the right, so the library keeps the whole middle and what is
  // playing never leaves the screen. The accent of the theme (or of the cover, when "Color from album art" is on)
  // washes down from the top of it.
  const entry = $derived(player.shown);
  const soon = $derived(player.upNext.slice(0, 3));
</script>

{#if entry}
  <aside class="stage" aria-label="Player" data-testid="player-bar">
    <button class="stage-art shrink-0" aria-label="Open Now Playing" onclick={() => nav.openNowPlaying()}>
      <Thumb src={entry.track.thumbnail} seed={entry.track.key} size={340} fill rounded="r-lg" />
    </button>
    <div class="min-w-0">
      <p class="t-subtitle truncate" title={entry.track.title}>{entry.track.title}</p>
      <p class="muted truncate">{entry.track.artist}</p>
    </div>
    {#if player.failure}
      <p class="accent-text flex items-center gap-2" role="alert">
        <span class="min-w-0 flex-1">{describeFailure(player.failure)}</span>
        <Button variant="primary" size="sm" onclick={player.retry}>Retry</Button>
      </p>
    {/if}
    <div class="t-caption muted nums flex items-center gap-2">
      <span class="w-10 text-right">{formatTime(player.state.positionMs)}</span>
      <div class="min-w-0 flex-1"><Seek positionMs={player.state.positionMs} durationMs={player.durationMs} onseek={player.seek} accent label="Seek" /></div>
      <span class="w-10">{player.durationMs ? formatTime(player.durationMs) : '--:--'}</span>
    </div>
    <div class="flex items-center justify-center gap-4">
      <IconButton icon="prev" label="Previous" size="lg" strong iconSize={24} onclick={player.previous} />
      <PlayButton size="lg" />
      <IconButton icon="next" label="Next" size="lg" strong iconSize={24} onclick={player.next} />
    </div>
    <div class="flex items-center justify-between gap-2">
      <LikeButton track={entry.track} />
      <div class="w-32"><VolumeSlider /></div>
      <IconButton icon="expand" label="Expand Now Playing" onclick={() => nav.openNowPlaying()} />
    </div>
    <div class="flex items-center justify-between pt-2">
      <span class="t-strong">Up next</span>
      <Button variant="quiet" size="sm" onclick={() => nav.go('queue')}>Open queue</Button>
    </div>
    {#if soon.length > 0}
      <ul>
        {#each soon as e (e.id)}
          <li>
            <MenuItem onclick={() => player.playEntry(e.id)}>
              <Thumb src={e.track.thumbnail} seed={e.track.key} size={32} rounded="r-sm" />
              <span class="min-w-0 flex-1">
                <span class="trow-title block">{e.track.title}</span>
                <span class="trow-artist block">{e.track.artist}</span>
              </span>
            </MenuItem>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="t-caption muted">Nothing queued after this song.</p>
    {/if}
  </aside>
{:else}
  <aside class="stage stage-empty" aria-label="Player">
    <Logo size={48} />
    <p class="t-strong mt-4">Nothing playing</p>
    <p class="max-w-56">Pick a song and it shows up here.</p>
  </aside>
{/if}
