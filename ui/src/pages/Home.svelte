<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import type { HomeDto } from '$lib/bindings/HomeDto';
  import EmptyState from '../components/EmptyState.svelte';
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
  const demo = $derived(!hasLibrary && player.catalog && player.catalog.length > 0 ? player.catalog : null);
</script>

<section class="px-4 pb-8 sm:px-8">
  {#if home === null}
    <h1 class="mb-4 text-2xl font-bold">Quick picks</h1>
    <Skeleton rows={8} />
  {:else if hasLibrary}
    {#if home.quickPicks.length > 0}
      <h1 class="mb-2 text-2xl font-bold">Quick picks</h1>
      <ul class="grid gap-x-6 md:grid-cols-2" aria-label="Quick picks">
        {#each home.quickPicks as t, i (t.key)}
          <TrackRow track={t} onplay={() => player.playTracks(home!.quickPicks, i)} />
        {/each}
      </ul>
    {/if}
    {#if home.recent.length > 0}
      <h2 class="mb-3 mt-8 text-2xl font-bold">Recently played</h2>
      <ul class="flex gap-4 overflow-x-auto pb-2" aria-label="Recently played">
        {#each home.recent as t, i (t.key)}
          <li class="w-36 shrink-0">
            <button class="block w-full text-left" aria-label="Play {t.title}" onclick={() => player.playTracks(home!.recent, i)}>
              <Thumb src={t.thumbnail} seed={t.key} size={144} rounded="rounded-lg" />
              <span class="mt-2 block truncate text-[15px] font-semibold">{t.title}</span>
              <span class="block truncate text-[13px] text-muted">{t.artist}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
    {#if home.likedCount > 0}
      <h2 class="mb-3 mt-8 text-2xl font-bold">Your likes</h2>
      <button class="rounded-xl bg-surface-2 px-5 py-4 text-left" onclick={() => { nav.libraryTab = 'liked'; nav.go('library'); }}>
        <span class="block text-[15px] font-semibold">Liked songs</span>
        <span class="text-[13px] text-muted">{home.likedCount} {home.likedCount === 1 ? 'song' : 'songs'}</span>
      </button>
    {/if}
  {:else if demo}
    <h1 class="text-2xl font-bold">Quick picks</h1>
    <p class="mb-4 mt-1 text-sm text-muted">Demo tracks with simulated playback. Real picks appear here as you listen and like songs.</p>
    <ul class="grid gap-x-6 md:grid-cols-2">
      {#each demo as t, i (t.key)}
        <TrackRow track={t} onplay={() => player.playTracks(demo, i)} />
      {/each}
    </ul>
  {:else}
    <EmptyState icon="music" title="Nothing here yet" text="Search for something to play. Songs you listen to and like will fill this page with quick picks." />
  {/if}
</section>
