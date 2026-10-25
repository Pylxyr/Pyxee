<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import type { HomeDto } from '$lib/bindings/HomeDto';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import Button from '../components/Button.svelte';
  import CoverGrid from '../components/CoverGrid.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import SectionHead from '../components/SectionHead.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import Thumb from '../components/Thumb.svelte';
  import TrackList from '../components/TrackList.svelte';

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

<ScrollPage>
  {#snippet header()}
    <PageHeader title="Home" />
  {/snippet}

  {#if home === null}
    <SectionHead>On rotation</SectionHead>
    <Skeleton rows={8} />
  {:else if hasLibrary}
    <div class="home-body">
    {#if hero}
      {@const button = HERO_BUTTON[hero.action]}
      <div class="hero" data-testid="resume">
        <Thumb src={hero.track.thumbnail} seed={hero.track.key} size={120} rounded="r-md" />
        <div class="min-w-0 flex-1">
          <p class="muted">{hero.label}</p>
          <p class="t-title truncate">{hero.track.title}</p>
          <p class="muted truncate">{hero.track.artist}</p>
          <Button class="mt-3" variant="primary" aria-label="{button.text} {hero.track.title}" onclick={() => heroClick(hero)}>
            {#if button.icon}<Icon name={button.icon} size={16} fill={button.icon === 'play'} />{/if}{button.text}
          </Button>
        </div>
        {#if home.likedCount > 0}
          <button class="hidden shrink-0 self-stretch r-md border border-line px-5 text-left hover:bg-surface-2 sm:block" onclick={() => { nav.libraryTab = 'liked'; nav.go('library'); }}>
            <span class="muted block">Liked songs</span>
            <span class="t-title nums mt-1 block">{home.likedCount}</span>
            <span class="t-caption muted">open the list</span>
          </button>
        {/if}
      </div>
    {/if}
    <div class="home-cols">
      {#if home.quickPicks.length > 0}
        <section class="min-w-0">
          <SectionHead count={home.quickPicks.length}>On rotation</SectionHead>
          <TrackList label="Quick picks" tracks={home.quickPicks} onplay={(i) => player.playTracks(home!.quickPicks, i)} />
        </section>
      {/if}
      {#if home.recent.length > 0}
        <section class="min-w-0">
          <SectionHead count={home.recent.length}>Recently played</SectionHead>
          <CoverGrid label="Recently played" tracks={home.recent} onplay={(i) => player.playTracks(home!.recent, i)} />
        </section>
      {/if}
    </div>
    </div>
  {:else if demo}
    <SectionHead count={demo.length}>On rotation</SectionHead>
    <p class="muted mb-2">Demo tracks with simulated playback. Real picks appear here as you listen and like songs.</p>
    <TrackList label="Demo tracks" tracks={demo} onplay={(i) => player.playTracks(demo, i)} />
  {:else}
    <EmptyState icon="music" title="Nothing here yet" text="Search for something to play. Songs you listen to and like will fill this page with quick picks." />
  {/if}
</ScrollPage>
