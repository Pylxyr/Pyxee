<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import type { HomeDto } from '$lib/bindings/HomeDto';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import SectionHead from '../components/SectionHead.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import Thumb from '../components/Thumb.svelte';
  import TrackRow from '../components/TrackRow.svelte';

  let home = $state<HomeDto | null>(null);

  // Quick picks and recents come from what you play and like; they refresh as the library changes.
  $effect(() => {
    void library.revision;
    library.backend?.homeGet().then((h) => (home = h)).catch(() => (home = { quickPicks: [], recent: [], likedCount: 0 }));
  });

  const hasLibrary = $derived(home !== null && (home.quickPicks.length > 0 || home.recent.length > 0));
  // The card at the top follows what is playing right now, the moment it changes. "Resume" is only offered when the
  // song is paused (including the one restored when the app starts); while it plays the button pauses it. With nothing
  // loaded it falls back to the last song in the history.
  type Hero = { track: TrackDto; label: string; action: 'pause' | 'resume' | 'retry' | 'replay' };
  const hero = $derived.by((): Hero | null => {
    const state = player.state.transport.state;
    const now = player.shown;
    if (now && state !== 'idle') {
      if (state === 'playing' || state === 'loading' || state === 'stalled') return { track: now.track, label: 'Now playing', action: 'pause' };
      if (state === 'failed') return { track: now.track, label: "Couldn't play this song", action: 'retry' };
      return { track: now.track, label: 'Pick up where you left off', action: 'resume' };
    }
    const last = home?.recent[0];
    return last ? { track: last, label: 'Pick up where you left off', action: 'replay' } : null;
  });
  const HERO_BUTTON = {
    pause: { text: 'Pause', icon: 'pause' },
    resume: { text: 'Resume', icon: 'play' },
    replay: { text: 'Resume', icon: 'play' },
    retry: { text: 'Try again', icon: null },
  } as const;
  function heroClick(h: Hero) {
    if (h.action === 'replay') void player.playTracks(home!.recent, 0);
    else if (h.action === 'retry') void player.retry();
    else void player.toggle();
  }
  const demo = $derived(!hasLibrary && player.catalog && player.catalog.length > 0 ? player.catalog : null);
</script>

<section class="h-full overflow-y-auto px-4 pb-8 pt-5 sm:px-6">
  {#if home === null}
    <SectionHead level={1}>On rotation</SectionHead>
    <Skeleton rows={8} />
  {:else if hasLibrary}
    {#if hero}
      {@const button = HERO_BUTTON[hero.action]}
      <div class="flex items-center gap-5 rounded border border-line bg-surface p-4" data-testid="resume">
        <Thumb src={hero.track.thumbnail} seed={hero.track.key} size={112} rounded="rounded-sm" />
        <div class="min-w-0 flex-1">
          <p class="label">{hero.label}</p>
          <p class="mt-1 truncate text-xl font-semibold">{hero.track.title}</p>
          <p class="truncate text-sm text-muted">{hero.track.artist}</p>
          <button class="mt-3 flex items-center gap-2 rounded bg-accent px-4 py-1.5 text-sm font-semibold text-on-accent" aria-label="{button.text} {hero.track.title}" onclick={() => heroClick(hero)}>
            {#if button.icon}<Icon name={button.icon} size={16} fill={button.icon === 'play'} />{/if}{button.text}
          </button>
        </div>
        {#if home.likedCount > 0}
          <button class="hidden shrink-0 self-stretch rounded border border-line px-5 text-left hover:bg-surface-2 sm:block" onclick={() => { nav.libraryTab = 'liked'; nav.go('library'); }}>
            <span class="label block">Liked songs</span>
            <span class="mt-1 block text-3xl font-semibold tabular-nums">{home.likedCount}</span>
            <span class="text-[12px] text-muted">open the list</span>
          </button>
        {/if}
      </div>
    {/if}
    {#if home.quickPicks.length > 0}
      <SectionHead level={1} count={home.quickPicks.length}>On rotation</SectionHead>
      <ul aria-label="Quick picks">
        {#each home.quickPicks as t, i (t.key)}
          <TrackRow track={t} index={i} onplay={() => player.playTracks(home!.quickPicks, i)} />
        {/each}
      </ul>
    {/if}
    {#if home.recent.length > 0}
      <SectionHead count={home.recent.length}>Recently played</SectionHead>
      <ul class="grid grid-cols-[repeat(auto-fill,minmax(112px,1fr))] gap-2 pt-2" aria-label="Recently played">
        {#each home.recent as t, i (t.key)}
          <li class="relative aspect-square overflow-hidden rounded-sm">
            <button class="group block h-full w-full text-left" aria-label="Play {t.title}" onclick={() => player.playTracks(home!.recent, i)}>
              <Thumb src={t.thumbnail} seed={t.key} size={160} fill rounded="rounded-sm" />
              <span class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/85 to-transparent px-2 pb-1.5 pt-6 text-white">
                <span class="block truncate text-[12px] font-semibold">{t.title}</span>
                <span class="block truncate text-[11px] opacity-75">{t.artist}</span>
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {:else if demo}
    <SectionHead level={1} count={demo.length}>On rotation</SectionHead>
    <p class="mb-2 text-sm text-muted">Demo tracks with simulated playback. Real picks appear here as you listen and like songs.</p>
    <ul>
      {#each demo as t, i (t.key)}
        <TrackRow track={t} index={i} onplay={() => player.playTracks(demo, i)} />
      {/each}
    </ul>
  {:else}
    <EmptyState icon="music" title="Nothing here yet" text="Search for something to play. Songs you listen to and like will fill this page with quick picks." />
  {/if}
</section>
