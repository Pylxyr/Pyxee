<script lang="ts">
  import { fly } from 'svelte/transition';
  import { player, describeFailure } from '$lib/player.svelte';
  import { nav, type NowPlayingTab } from '$lib/nav.svelte';
  import { removeFromQueue } from '$lib/actions';
  import { formatTime } from '$lib/format';
  import { reorder } from '$lib/reorder';
  import { MOTION, motionMs, motionPx } from '$lib/motion';
  import { sleep } from '$lib/sleep.svelte';
  import { sleepLabel } from '$lib/lyrics';
  import Button from './Button.svelte';
  import EditableRow from './EditableRow.svelte';
  import Icon from './Icon.svelte';
  import IconButton from './IconButton.svelte';
  import LikeButton from './LikeButton.svelte';
  import LyricsPanel from './LyricsPanel.svelte';
  import PlayButton from './PlayButton.svelte';
  import RelatedPanel from './RelatedPanel.svelte';
  import Seek from './Seek.svelte';
  import Segmented from './Segmented.svelte';
  import Thumb from './Thumb.svelte';

  const entry = $derived(player.shown);
  const tabs: { id: NowPlayingTab; label: string }[] = [
    { id: 'upnext', label: 'Up next' },
    { id: 'lyrics', label: 'Lyrics' },
    { id: 'related', label: 'Related' },
  ];
  let drag = $state<{ id: number; from: number; to: number } | null>(null);
  let upNextEl = $state<HTMLElement>();
  const upNextRows = () => [...(upNextEl?.querySelectorAll<HTMLElement>('[data-row]') ?? [])];
  const firstRadioId = $derived(player.upNext.find((e) => e.origin === 'radio')?.id ?? null);
  const overOf = (id: number, i: number): 'above' | 'below' | null =>
    drag && drag.to === i && drag.id !== id ? (drag.to < drag.from ? 'above' : 'below') : null;
</script>

{#if nav.nowPlayingOpen && entry}
  <div
    class="fixed inset-0 z-40 flex flex-col overflow-hidden"
    style="top:var(--titlebar-h, 0px);background:color-mix(in srgb, var(--accent) 14%, var(--solid))"
    data-testid="np-root"
    role="region"
    aria-label="Now Playing"
    transition:fly={{ y: motionPx(80), duration: motionMs(MOTION.base) }}
  >
    <div class="page-pad flex shrink-0 items-center gap-2 py-3" data-testid="np-header">
      <IconButton icon="down" label="Close Now Playing" size="lg" strong onclick={() => nav.closeNowPlaying()} />
      <span class="t-strong muted">Now playing</span>
    </div>

    <div class="grid min-h-0 w-full flex-1 grid-cols-[minmax(0,1fr)] grid-rows-[auto_minmax(0,1fr)] gap-4 px-6 pb-4 lg:grid-cols-[minmax(0,1fr)_28rem] lg:grid-rows-1 lg:gap-10 lg:pb-8 xl:grid-cols-[minmax(0,1fr)_34rem] 2xl:grid-cols-[minmax(0,1fr)_44rem] 2xl:gap-16 2xl:px-10">
      <section class="flex min-h-0 flex-col items-center justify-center gap-3 lg:gap-5 lg:overflow-y-auto" data-testid="np-controls">
        <div class="stage-art aspect-square w-[min(56vw,26vh)] shrink-0 lg:w-[min(22rem,48vh)] xl:w-[min(30rem,50vh)] 2xl:w-[min(40rem,56vh)]" data-testid="np-art">
          <Thumb src={entry.track.thumbnail} seed={entry.track.key} size={352} fill rounded="r-lg" />
        </div>
        <div class="flex w-[min(80vw,22rem)] items-center justify-between gap-3 xl:w-[min(80vw,30rem)] 2xl:w-[min(80vw,40rem)]">
          <div class="min-w-0">
            <h1 class="t-title truncate">{entry.track.title}</h1>
            <p class="muted truncate">{entry.track.artist}</p>
          </div>
          <div class="flex items-center">
            <IconButton icon="moon" label="Sleep timer" class="w-auto gap-1 px-2 {sleep.status ? 'text-accent' : ''}" onclick={() => (nav.sleepSheetOpen = true)}>
              {#if sleep.status}<span class="t-caption nums font-semibold" data-testid="sleep-chip">{sleepLabel(sleep.status)}</span>{/if}
            </IconButton>
            <LikeButton track={entry.track} />
          </div>
        </div>
        <div class="w-[min(80vw,22rem)] xl:w-[min(80vw,30rem)] 2xl:w-[min(80vw,40rem)]">
          <Seek positionMs={player.state.positionMs} durationMs={player.durationMs} onseek={player.seek} accent label="Seek" />
          <div class="t-caption muted nums flex justify-between">
            <span>{formatTime(player.state.positionMs)}</span><span>{player.durationMs ? formatTime(player.durationMs) : 'length unknown'}</span>
          </div>
        </div>
        {#if player.failure}
          <p class="accent-text" role="alert">
            {describeFailure(player.failure)}
            <button class="btn-link ml-2" onclick={player.retry}>Retry</button>
          </p>
        {/if}
        <div class="flex items-center gap-4">
          <IconButton icon="shuffle" label="Shuffle" size="lg" pressed={player.state.shuffle} onclick={player.toggleShuffle} />
          <IconButton icon="prev" label="Previous" size="lg" strong iconSize={26} onclick={player.previous} />
          <PlayButton size="lg" announceLoading={false} />
          <IconButton icon="next" label="Next" size="lg" strong iconSize={26} onclick={player.next} />
          <IconButton icon="repeat" label="Repeat: {player.state.repeat}" size="lg" class={player.state.repeat !== 'off' ? 'text-accent' : ''} onclick={player.cycleRepeat}>
            {#if player.state.repeat === 'one'}<span class="absolute right-1 top-1 text-[10px] font-semibold">1</span>{/if}
          </IconButton>
        </div>
      </section>

      <section class="flex min-h-0 min-w-0 flex-col">
        <div class="flex shrink-0 flex-wrap items-center justify-between gap-x-3 gap-y-1" data-testid="np-tabs">
          <Segmented items={tabs} value={nav.tab} onselect={(id) => (nav.tab = id)} label="Now Playing sections" class="min-w-0 flex-nowrap [&_.segment]:whitespace-nowrap [&_.segment]:px-2.5" />
          <Button variant="quiet" size="sm" class="shrink-0" onclick={() => { nav.closeNowPlaying(); nav.go('queue'); }}>Open queue</Button>
        </div>
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div class="mt-3 min-h-0 flex-1 overflow-y-auto" role="tabpanel" data-testid="np-scroll" data-scroll tabindex="0">
          {#if nav.tab === 'upnext'}
            <ul class="space-y-0.5" bind:this={upNextEl}>
              <li class="row-edit px-2 py-1.5" data-current="true">
                <Thumb src={entry.track.thumbnail} seed={entry.track.key} size={44} rounded="r-sm" />
                <div class="ml-3 min-w-0 flex-1">
                  <div class="trow-title accent-text">{entry.track.title}</div>
                  <div class="trow-artist">Now playing</div>
                </div>
                <span class="trow-time">{formatTime(entry.track.durationMs)}</span>
              </li>
              {#each player.upNext as e, i (e.id)}
                {#if e.id === firstRadioId}
                  <li class="t-caption muted px-3 pb-1 pt-4">Autoplay · similar songs</li>
                {/if}
                <EditableRow
                  data-row
                  track={e.track}
                  coverSize={44}
                  over={overOf(e.id, i)}
                  dragging={drag?.id === e.id}
                  onplay={() => player.playEntry(e.id)}
                >
                  {#snippet handle()}
                    <span
                      class="handle"
                      role="presentation"
                      title="Drag to reorder"
                      data-testid="upnext-handle"
                      use:reorder={{
                        index: i,
                        rows: upNextRows,
                        onover: (to) => (drag = to === null ? null : { id: e.id, from: i, to }),
                        ondrop: (to) => player.moveUpcoming(e.id, to),
                      }}><Icon name="drag" size={18} /></span>
                  {/snippet}
                  {#snippet actions()}
                    <IconButton icon="top" iconSize={18} label="Play {e.track.title} next" title="Play next" disabled={i === 0} onclick={() => player.moveUpcoming(e.id, 0)} />
                    <IconButton icon="up" iconSize={18} label="Move {e.track.title} up" disabled={i === 0} onclick={() => player.moveUpcoming(e.id, i - 1)} />
                    <IconButton icon="down" iconSize={18} label="Move {e.track.title} down" disabled={i === player.upNext.length - 1} onclick={() => player.moveUpcoming(e.id, i + 1)} />
                    <IconButton icon="close" iconSize={18} label="Remove {e.track.title} from queue" onclick={() => removeFromQueue(e)} />
                  {/snippet}
                </EditableRow>
              {:else}
                <li class="muted px-3 py-6">Nothing queued after this track.</li>
              {/each}
            </ul>
          {:else if nav.tab === 'lyrics'}
            <LyricsPanel active={nav.tab === 'lyrics'} />
          {:else}
            <RelatedPanel active={nav.tab === 'related'} />
          {/if}
        </div>
      </section>
    </div>
  </div>
{/if}
