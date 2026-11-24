<script lang="ts">
  import { player, describeFailure } from '$lib/player.svelte';
  import { nav } from '$lib/nav.svelte';
  import { artistOf, formatTime, titleOf } from '$lib/format';
  import { flipList } from '$lib/flip';
  import { reorder } from '$lib/reorder';
  import { openTrackMenu } from '$lib/menu';
  import { queueMenu } from '$lib/actions';
  import Button from './Button.svelte';
  import IconButton from './IconButton.svelte';
  import LikeButton from './LikeButton.svelte';
  import Logo from './Logo.svelte';
  import MenuItem from './MenuItem.svelte';
  import PlayButton from './PlayButton.svelte';
  import Seek from './Seek.svelte';
  import SeekWake from './SeekWake.svelte';
  import Thumb from './Thumb.svelte';
  import VolumeSlider from './VolumeSlider.svelte';

  // The player of a wide window, in two parts that the shell places on its grid: the deck along the bottom (what is
  // playing, the transport, and a seek line that runs the full width of the window), and from 1440 px the queue lane
  // beside the page, so what comes next stays in sight while you browse.
  // `lane`: the lane has its column (a window of 1440 px or more, and the person has not hidden it). In a narrower
  // window the queue button on the deck opens it as a drawer over the page's right edge.
  let { lane = false, room = false }: { lane?: boolean; room?: boolean } = $props();
  const drawer = $derived(!lane && nav.queueDrawer);
  const queueShown = $derived(lane || drawer);
  function toggleQueue() {
    if (room) nav.laneHidden = !nav.laneHidden;
    else nav.queueDrawer = !nav.queueDrawer;
  }
  const entry = $derived(player.shown);
  // Where the seek bar's thumb is held (dragged, or released and waiting for the player): the time shows it too.
  let scrub = $state<number | null>(null);

  // Up next can be rearranged by dragging a row itself, with no handle: a click still plays it, a drag moves it.
  let drag = $state<{ id: number; from: number; to: number } | null>(null);
  let laneEl = $state<HTMLElement>();
  const laneRows = () => [...(laneEl?.querySelectorAll<HTMLElement>('[data-lane-row]') ?? [])];
  const overOf = (id: number, i: number): 'above' | 'below' | null =>
    drag && drag.to === i && drag.id !== id ? (drag.to < drag.from ? 'above' : 'below') : null;

  // Escape closes the drawer wherever the focus is (opening it from the deck leaves the focus on the deck's button, not in
  // the drawer). A pop-up menu or a sheet on top of it takes the key first and closes itself.
  function closeDrawerOnEscape(e: KeyboardEvent) {
    if (!drawer || e.key !== 'Escape' || e.defaultPrevented) return;
    if (document.querySelector('[data-sheet], [data-testid="ctx-menu"]')) return;
    e.preventDefault();
    nav.queueDrawer = false;
  }

  // The bar's empty space opens the queue; the buttons and sliders in it keep their own jobs.
  function openQueue(e: MouseEvent) {
    if ((e.target as HTMLElement).closest('button, input, [role="slider"]')) return;
    nav.go('queue');
  }
</script>

<svelte:window onkeydown={closeDrawerOnEscape} />

{#if entry && queueShown}
  {#if drawer}<button class="lane-scrim" aria-label="Close the queue" tabindex="-1" onclick={() => (nav.queueDrawer = false)}></button>{/if}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <aside class="lane" data-drawer={drawer ? '' : undefined} aria-label="Up next" data-testid="queue-lane" onkeydown={(e) => { if (drawer && e.key === 'Escape') { e.stopPropagation(); nav.queueDrawer = false; } }}>
    <div class="lane-head">
      <h2 class="t-lead">Up next <span class="lane-count nums">{player.upNext.length}</span></h2>
      <span class="lane-head-tools">
        <Button variant="quiet" size="sm" onclick={() => { nav.queueDrawer = false; nav.go('queue'); }}>Open queue</Button>
        <IconButton icon="close" iconSize={18} label="Hide the queue" onclick={toggleQueue} />
      </span>
    </div>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <ul class="lane-list" data-scroll tabindex="0" use:flipList bind:this={laneEl}>
      {#key entry.track.key}
      <li class="lane-now track-in" oncontextmenu={(ev) => { const e0 = player.shown; if (e0) openTrackMenu(e0.track, { event: ev, extras: queueMenu(e0), queue: true }); }}>
        <Thumb src={entry.track.thumbnail} seed={entry.track.key} size={40} rounded="r-sm" />
        <span class="min-w-0 flex-1">
          <span class="trow-title accent-text block" dir="auto">{titleOf(entry.track)}</span>
          <span class="trow-artist block">Now playing</span>
        </span>
        <span class="trow-time">{formatTime(entry.track.durationMs)}</span>
      </li>
      {/key}
      {#each player.upNext as e, i (e.id)}
        <li
          data-lane-row
          data-over={overOf(e.id, i)}
          data-dragging={drag?.id === e.id ? 'true' : undefined}
          use:reorder={{
            index: i,
            rows: laneRows,
            threshold: 5,
            onover: (to) => (drag = to === null ? null : { id: e.id, from: i, to }),
            ondrop: (to) => player.moveUpcoming(e.id, to),
          }}
          oncontextmenu={(ev) => openTrackMenu(e.track, { event: ev, opener: (ev.currentTarget as HTMLElement).querySelector('button'), extras: queueMenu(e), queue: true })}>
          <MenuItem onclick={() => player.playEntry(e.id)}>
            <Thumb src={e.track.thumbnail} seed={e.track.key} size={40} rounded="r-sm" />
            <span class="min-w-0 flex-1">
              <span class="trow-title block" dir="auto">{titleOf(e.track)}</span>
              <span class="trow-artist block" dir="auto">{artistOf(e.track)}</span>
            </span>
            <span class="trow-time">{formatTime(e.track.durationMs)}</span>
          </MenuItem>
        </li>
      {:else}
        <li class="t-caption muted px-2 py-3">Nothing queued after this song.</li>
      {/each}
    </ul>
  </aside>
{:else if lane}
  <!-- Nothing is playing: the column stays, so the page does not jump sideways when the first song starts. -->
  <aside class="lane" aria-label="Queue" data-testid="queue-lane-idle">
    <div class="lane-head">
      <h2 class="t-lead">Up next</h2>
      <IconButton icon="close" iconSize={18} label="Hide the queue" onclick={toggleQueue} />
    </div>
    <div class="lane-idle">
      <div class="lane-idle-disc"><Logo size={44} /></div>
      <p class="t-strong">Nothing queued yet</p>
      <p class="t-caption">Pick a song and what plays next shows up here. Press <kbd>/</kbd> to search.</p>
    </div>
  </aside>
{/if}

{#if entry}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <section class="deck" aria-label="Player" data-testid="player-bar" onclick={openQueue}>
    <div class="deck-seek">
      <Seek positionMs={player.positionMs} durationMs={player.durationMs} onseek={player.seek} accent label="Seek" bind:scrub />
      <SeekWake songKey={entry.track.key} />
    </div>
    {#key entry.track.key}
    <div class="deck-now track-in">
      <button class="deck-art" data-np-source data-paused={!player.isPlaying} aria-label="Open Now Playing" onclick={() => nav.openNowPlaying()}>
        <Thumb src={entry.track.thumbnail} seed={entry.track.key} size={72} fill rounded="r-sm" />
      </button>
      <div class="min-w-0 flex-1">
        <p class="t-strong truncate" dir="auto" title={titleOf(entry.track)}>{titleOf(entry.track)}</p>
        {#if player.failure}
          <p class="t-caption accent-text fail-in flex items-center gap-2" role="alert">
            <span class="min-w-0 flex-1 truncate">{describeFailure(player.failure)}</span>
            <Button variant="primary" size="sm" onclick={player.retry}>Retry</Button>
          </p>
        {:else}
          <p class="muted truncate" dir="auto">{artistOf(entry.track)}</p>
        {/if}
      </div>
      <LikeButton track={entry.track} />
    </div>
    {/key}
    <div class="deck-transport">
      <IconButton icon="prev" label="Previous" size="lg" strong iconSize={24} onclick={player.previous} />
      <PlayButton />
      <IconButton icon="next" label="Next" size="lg" strong iconSize={24} onclick={player.next} />
      <span class="deck-time nums">{formatTime(scrub ?? player.positionMs)} / {player.durationMs ? formatTime(player.durationMs) : '--:--'}</span>
    </div>
    <div class="deck-extra">
      <span class="deck-pair">
        <IconButton icon="shuffle" label="Shuffle" iconSize={18} pressed={player.state.shuffle} onclick={player.toggleShuffle} />
        <IconButton icon="repeat" label="Repeat: {player.state.repeat}" iconSize={18} class={player.state.repeat !== 'off' ? 'text-accent' : ''} onclick={player.cycleRepeat}>
          {#if player.state.repeat === 'one'}<span class="absolute right-1 top-1 text-[10px] font-semibold">1</span>{/if}
        </IconButton>
      </span>
      <IconButton icon="lyrics" label="Lyrics" iconSize={18} onclick={() => { nav.tab = 'lyrics'; nav.openNowPlaying(); }} />
      {#if nav.route !== 'queue'}<IconButton icon="queue" label={queueShown ? 'Hide the queue' : 'Show the queue'} iconSize={18} pressed={queueShown} onclick={toggleQueue} />{/if}
      <div class="deck-volume"><VolumeSlider /></div>
      <IconButton icon="expand" label="Expand Now Playing" onclick={() => nav.openNowPlaying()} />
    </div>
  </section>
{:else}
  <section class="deck deck-empty" aria-label="Player">
    <Logo size={24} />
    <p class="muted">Nothing playing. Pick a song and it shows up here.</p>
  </section>
{/if}
