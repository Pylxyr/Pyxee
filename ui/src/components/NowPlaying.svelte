<script lang="ts">
  import { fly } from 'svelte/transition';
  import { player, describeFailure } from '$lib/player.svelte';
  import { nav, type NowPlayingTab } from '$lib/nav.svelte';
  import { library } from '$lib/library.svelte';
  import { removeFromQueue } from '$lib/actions';
  import { formatTime } from '$lib/format';
  import { reorder } from '$lib/reorder';
  import Icon from './Icon.svelte';
  import Thumb from './Thumb.svelte';
  import Seek from './Seek.svelte';
  import LyricsPanel from './LyricsPanel.svelte';
  import { sleep } from '$lib/sleep.svelte';
  import { sleepLabel } from '$lib/lyrics';

  const entry = $derived(player.shown);
  const liked = $derived(entry ? library.isLiked(entry.track.key) : false);
  const tabs: { id: NowPlayingTab; label: string }[] = [
    { id: 'upnext', label: 'Up next' },
    { id: 'lyrics', label: 'Lyrics' },
    { id: 'related', label: 'Related' },
  ];
  let drag = $state<{ id: number; from: number; to: number } | null>(null);
  let upNextEl = $state<HTMLElement>();
  const upNextRows = () => [...(upNextEl?.querySelectorAll<HTMLElement>('[data-row]') ?? [])];
  const firstRadioId = $derived(player.upNext.find((e) => e.origin === 'radio')?.id ?? null);
</script>

{#if nav.nowPlayingOpen && entry}
  <div
    class="fixed inset-0 z-40 flex flex-col overflow-hidden"
    style="background:color-mix(in srgb, var(--accent) 12%, var(--solid))"
    data-testid="np-root"
    role="region"
    aria-label="Now Playing"
    transition:fly={{ y: 80, duration: 200 }}
  >
    <div class="flex shrink-0 items-center gap-3 px-4 py-3 sm:px-6" data-testid="np-header">
      <button class="p-2" aria-label="Close Now Playing" onclick={() => nav.closeNowPlaying()}><Icon name="down" /></button>
      <span class="text-sm font-semibold text-muted">Now playing</span>
    </div>

    <div class="mx-auto grid min-h-0 w-full max-w-6xl flex-1 grid-rows-[auto_minmax(0,1fr)] gap-4 px-6 pb-4 lg:grid-cols-[1fr_28rem] lg:grid-rows-1 lg:gap-8 lg:pb-8">
      <section class="flex min-h-0 flex-col items-center justify-center gap-3 lg:gap-5 lg:overflow-y-auto" data-testid="np-controls">
        <div class="aspect-square w-[min(56vw,26vh)] shrink-0 overflow-hidden rounded-xl lg:w-[min(22rem,48vh)]" data-testid="np-art">
          <Thumb src={entry.track.thumbnail} seed={entry.track.key} size={352} fill rounded="rounded-xl" />
        </div>
        <div class="flex w-[min(80vw,22rem)] items-center justify-between gap-3">
          <div class="min-w-0">
            <h1 class="truncate text-2xl font-bold">{entry.track.title}</h1>
            <p class="truncate text-muted">{entry.track.artist}</p>
          </div>
          <button class="flex items-center gap-1 p-2 {sleep.status ? 'text-accent' : 'text-muted hover:text-fg'}" aria-label="Sleep timer" onclick={() => (nav.sleepSheetOpen = true)}>
            <Icon name="moon" />
            {#if sleep.status}<span class="text-[12px] font-semibold" data-testid="sleep-chip">{sleepLabel(sleep.status)}</span>{/if}
          </button>
          <button class="p-2 {liked ? 'text-accent' : 'text-muted hover:text-fg'}" aria-label={liked ? 'Unlike' : 'Like'} aria-pressed={liked} onclick={() => entry && library.toggleLike(entry.track)}><Icon name="heart" fill={liked} /></button>
        </div>
        <div class="w-[min(80vw,22rem)]">
          <Seek positionMs={player.state.positionMs} durationMs={player.durationMs} onseek={player.seek} accent label="Seek" />
          <div class="mt-1 flex justify-between text-[13px] tabular-nums text-muted">
            <span>{formatTime(player.state.positionMs)}</span><span>{player.durationMs ? formatTime(player.durationMs) : 'length unknown'}</span>
          </div>
        </div>
        {#if player.failure}
          <p class="text-sm text-accent" role="alert">
            {describeFailure(player.failure)}
            <button class="ml-2 font-semibold underline" onclick={player.retry}>Retry</button>
          </p>
        {/if}
        <div class="flex items-center gap-6">
          <button class="p-2 {player.state.shuffle ? 'text-accent' : 'text-muted'}" aria-label="Shuffle" aria-pressed={player.state.shuffle} onclick={player.toggleShuffle}><Icon name="shuffle" /></button>
          <button class="p-2" aria-label="Previous" onclick={player.previous}><Icon name="prev" size={26} /></button>
          <button class="flex h-16 w-16 items-center justify-center rounded-full bg-fg text-on-fg" aria-label={player.isPlaying ? 'Pause' : 'Play'} onclick={player.toggle}>
            {#if player.state.transport.state === 'loading'}
              <span class="h-6 w-6 animate-spin rounded-full border-2 border-on-fg/30 border-t-on-fg" aria-hidden="true"></span>
            {:else}
              <Icon name={player.isPlaying ? 'pause' : 'play'} size={28} fill={!player.isPlaying} />
            {/if}
          </button>
          <button class="p-2" aria-label="Next" onclick={player.next}><Icon name="next" size={26} /></button>
          <button class="relative p-2 {player.state.repeat !== 'off' ? 'text-accent' : 'text-muted'}" aria-label="Repeat: {player.state.repeat}" onclick={player.cycleRepeat}>
            <Icon name="repeat" />
            {#if player.state.repeat === 'one'}<span class="absolute right-0.5 top-0 text-[10px] font-bold">1</span>{/if}
          </button>
        </div>
      </section>

      <section class="flex min-h-0 flex-col">
        <div class="flex shrink-0 gap-6 border-b border-line" role="tablist" aria-label="Now Playing sections" data-testid="np-tabs">
          {#each tabs as t (t.id)}
            <button
              role="tab"
              aria-selected={nav.tab === t.id}
              class="-mb-px border-b-2 pb-3 pt-2 text-[15px] font-semibold {nav.tab === t.id ? 'border-accent text-fg' : 'border-transparent text-muted'}"
              onclick={() => (nav.tab = t.id)}>{t.label}</button>
          {/each}
          <button class="-mb-px ml-auto pb-3 pt-2 text-sm font-semibold text-muted hover:text-fg" onclick={() => { nav.closeNowPlaying(); nav.go('queue'); }}>Open queue</button>
        </div>
        <div class="mt-3 min-h-0 flex-1 overflow-y-auto" role="tabpanel" data-testid="np-scroll">
          {#if nav.tab === 'upnext'}
            <ul class="space-y-0.5" bind:this={upNextEl}>
              <li class="flex items-center gap-3 rounded-lg bg-surface-2 px-3 py-2">
                <Thumb src={entry.track.thumbnail} seed={entry.track.key} size={44} />
                <div class="min-w-0 flex-1">
                  <div class="truncate text-[15px] font-semibold">{entry.track.title}</div>
                  <div class="truncate text-[13px] text-accent">Now playing</div>
                </div>
                <span class="text-[13px] tabular-nums text-muted">{formatTime(entry.track.durationMs)}</span>
              </li>
              {#each player.upNext as e, i (e.id)}
                {#if e.id === firstRadioId}
                  <li class="px-3 pb-1 pt-4 text-xs font-semibold uppercase tracking-wider text-muted">Autoplay · similar songs</li>
                {/if}
                <li
                  data-row
                  class="group flex items-center gap-1 rounded-lg border-y-2 border-transparent hover:bg-surface-2 {drag && drag.to === i && drag.id !== e.id ? (drag.to < drag.from ? 'border-t-accent' : 'border-b-accent') : ''} {drag?.id === e.id ? 'opacity-40' : ''}"
                >
                  <span
                    class="cursor-grab touch-none select-none py-2 pl-2 text-muted"
                    role="presentation"
                    title="Drag to reorder"
                    data-testid="upnext-handle"
                    use:reorder={{
                      index: i,
                      rows: upNextRows,
                      onover: (to) => (drag = to === null ? null : { id: e.id, from: i, to }),
                      ondrop: (to) => player.moveUpcoming(e.id, to),
                    }}><Icon name="drag" size={18} /></span>
                  <button class="flex min-w-0 flex-1 items-center gap-3 px-2 py-2 text-left" onclick={() => player.playEntry(e.id)}>
                    <Thumb src={e.track.thumbnail} seed={e.track.key} size={44} />
                    <span class="min-w-0 flex-1">
                      <span class="block truncate text-[15px] font-semibold">{e.track.title}</span>
                      <span class="block truncate text-[13px] text-muted">{e.track.artist}</span>
                    </span>
                    <span class="text-[13px] tabular-nums text-muted">{formatTime(e.track.durationMs)}</span>
                  </button>
                  <button class="p-1.5 text-muted hover:text-fg disabled:opacity-30" aria-label="Play {e.track.title} next" title="Play next" disabled={i === 0} onclick={() => player.moveUpcoming(e.id, 0)}><Icon name="top" size={18} /></button>
                  <button class="p-1.5 text-muted hover:text-fg disabled:opacity-30" aria-label="Move {e.track.title} up" disabled={i === 0} onclick={() => player.moveUpcoming(e.id, i - 1)}><Icon name="up" size={18} /></button>
                  <button class="p-1.5 text-muted hover:text-fg disabled:opacity-30" aria-label="Move {e.track.title} down" disabled={i === player.upNext.length - 1} onclick={() => player.moveUpcoming(e.id, i + 1)}><Icon name="down" size={18} /></button>
                  <button class="p-1.5 text-muted hover:text-fg" aria-label="Remove {e.track.title} from queue" onclick={() => removeFromQueue(e)}><Icon name="close" size={18} /></button>
                </li>
              {:else}
                <li class="px-3 py-6 text-sm text-muted">Nothing queued after this track.</li>
              {/each}
            </ul>
          {:else if nav.tab === 'lyrics'}
            <LyricsPanel active={nav.tab === 'lyrics'} />
          {:else}
            <p class="px-3 py-8 text-sm text-muted">Related songs arrive with search in milestone 4.</p>
          {/if}
        </div>
      </section>
    </div>
  </div>
{/if}
