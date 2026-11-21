<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { playWithRelated } from '$lib/radio';
  import { Loaded } from '$lib/loaded.svelte';
  import { heroTones } from '$lib/color';
  import { fillRows } from '$lib/fillrows';
  import { coverColorsOf } from '$lib/covercolors.svelte';
  import { artistOf, titleOf } from '$lib/format';
  import { look } from '$lib/look.svelte';
  import type { HomeDto } from '$lib/bindings/HomeDto';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import Backdrop from '../components/Backdrop.svelte';
  import Button from '../components/Button.svelte';
  import CoverGrid from '../components/CoverGrid.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import MixCard from '../components/MixCard.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import SectionHead from '../components/SectionHead.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import Thumb from '../components/Thumb.svelte';
  import TrackList from '../components/TrackList.svelte';

  // A greeting for the time of day (the hour is read once, when the page opens).
  const hour = new Date().getHours();
  const greeting = hour < 5 ? 'Still up?' : hour < 12 ? 'Good morning' : hour < 18 ? 'Good afternoon' : 'Good evening';

  // Quick picks and recents come from what you play and like; they refresh as the library changes. A late answer
  // never replaces a newer one, and a failed read keeps what is shown (with nothing shown yet, the page is just empty).
  const source = new Loaded<HomeDto>();
  const home = $derived<HomeDto | null>(source.data ?? (source.error ? { quickPicks: [], recent: [], likedCount: 0 } : null));
  $effect(() => {
    void library.revision;
    const b = library.backend;
    if (b) void source.load(() => b.homeGet());
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
  // The feature card's looks other than the blur are painted in the cover's colours. They are read only for those looks.
  const heroColors = coverColorsOf(() => (look.value.hero === 'blur' || look.value.hero === 'plain' ? null : (hero?.track.thumbnail ?? null)));
  const heroStyle = $derived.by(() => {
    const t = heroColors.value ? heroTones(heroColors.value) : null;
    return t ? `--hero-a: ${t.a}; --hero-b: ${t.b}; --hero-c: ${t.c}` : undefined;
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
  // "Tune in": the songs you come back to as cards, each one the start of a radio (the song, then songs like it). The
  // first card picks one of them at random.
  // Eleven songs and the surprise card; `fillRows` hides the few that would leave a short last row at the current width.
  const MIX_CARDS = 11;
  const seeds = $derived.by((): TrackDto[] => {
    const seen = new Set<string>();
    const out: TrackDto[] = [];
    for (const t of [...(home?.quickPicks ?? []), ...(home?.recent ?? [])]) {
      if (seen.has(t.key)) continue;
      seen.add(t.key);
      out.push(t);
      if (out.length >= MIX_CARDS) break;
    }
    return out;
  });
  function surprise() {
    const pool = [...(home?.quickPicks ?? []), ...(home?.recent ?? [])];
    const pick = pool[Math.floor(Math.random() * pool.length)];
    if (pick) void playWithRelated(pick);
  }
  const demo = $derived(!hasLibrary && player.catalog && player.catalog.length > 0 ? player.catalog : null);
</script>

<ScrollPage>
  {#snippet header()}
    <PageHeader title="Home" kicker={greeting} />
  {/snippet}

  {#if home === null}
    <SectionHead>Tune in</SectionHead>
    <Skeleton rows={8} />
  {:else if hasLibrary}
    <div class="home-body shelves">
    {#if hero}
      {@const button = HERO_BUTTON[hero.action]}
      <div
        class="hero"
        data-hero={look.value.hero}
        style={heroStyle}
        data-testid="resume"
        role="presentation"
        oncontextmenu={(e) => {
          e.preventDefault();
          nav.menuOpener = e.currentTarget.querySelector<HTMLElement>('button');
          nav.menuTrack = hero.track;
        }}
      >
        {#if look.value.hero === 'blur'}<Backdrop class="hero-bg" src={hero.track.thumbnail} seed={hero.track.key} />{/if}
        <div class="hero-stage">
          {#if look.value.hero === 'vinyl'}
            {#key hero.track.key}
              <span class="hero-disc" aria-hidden="true">
                <span class="hero-disc-label"><Thumb src={hero.track.thumbnail} seed={hero.track.key} size={72} fill rounded="r-full" /></span>
              </span>
            {/key}
          {/if}
          <div class="hero-art"><Thumb src={hero.track.thumbnail} seed={hero.track.key} size={240} fill rounded="r-sm" /></div>
        </div>
        <div class="min-w-0 flex-1">
          <p class="hero-label">{hero.label}</p>
          <p class="t-title truncate" dir="auto">{titleOf(hero.track)}</p>
          <p class="muted truncate" dir="auto">{artistOf(hero.track)}</p>
          <div class="mt-4 flex flex-wrap items-center gap-2">
            <Button variant="primary" aria-label="{button.text} {hero.track.title}" onclick={() => heroClick(hero)}>
              {#if button.icon}<Icon name={button.icon} size={16} fill={button.icon === 'play'} />{/if}{button.text}
            </Button>
            {#if home.likedCount > 0}
              <Button onclick={() => { nav.libraryTab = 'liked'; nav.go('library'); }}>Liked songs <span class="nums muted">{home.likedCount}</span></Button>
            {/if}
          </div>
        </div>
      </div>
    {/if}
    <div class="home-cols">
      {#if seeds.length > 0}
        <section class="min-w-0">
          <SectionHead>Tune in</SectionHead>
          <p class="muted mb-3">Pick one and Pyxee queues songs like it.</p>
          <ul class="mix-grid" aria-label="Start a radio" use:fillRows>
            <li><MixCard onpick={surprise} /></li>
            {#each seeds as t (t.key)}
              <li><MixCard track={t} onpick={() => void playWithRelated(t)} /></li>
            {/each}
          </ul>
        </section>
      {/if}
      {#if home.recent.length > 0}
        <section class="min-w-0">
          <SectionHead count={home.recent.length}>Recently played</SectionHead>
          <CoverGrid label="Recently played" tracks={home.recent} onplay={(i) => void playWithRelated(home!.recent[i]!)} />
        </section>
      {/if}
    </div>
    </div>
  {:else if demo}
    <SectionHead count={demo.length}>Demo songs</SectionHead>
    <p class="muted mb-2">Demo tracks with simulated playback. Real picks appear here as you listen and like songs.</p>
    <TrackList label="Demo tracks" tracks={demo} onplay={(i) => player.playTracks(demo, i)} />
  {:else}
    <EmptyState icon="music" title="Nothing here yet" text="Search for something to play. Songs you listen to and like will fill this page with quick picks." />
  {/if}
</ScrollPage>
