<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { search } from '$lib/search.svelte';
  import { explore } from '$lib/explore.svelte';
  import { MOODS } from '$lib/explore';
  import Icon from '../components/Icon.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import Thumb from '../components/Thumb.svelte';
  import TrackRow from '../components/TrackRow.svelte';

  // Loaded once per session (the shelves cost a search each); Refresh asks again.
  $effect(() => {
    const backend = library.backend;
    if (backend && library.ready) void explore.ensure(backend);
  });

  function mood(query: string) {
    nav.go('search');
    void search.run(query, 'songs');
  }
</script>

<section class="h-full overflow-y-auto px-4 pb-8 sm:px-8" data-scroll aria-labelledby="explore-title">
  <div class="flex items-end justify-between gap-3 pb-1">
    <div>
      <p class="text-[11px] font-semibold uppercase tracking-[0.2em] text-accent">Discover</p>
      <h1 id="explore-title" class="text-2xl font-semibold tracking-tight">Explore</h1>
    </div>
    <button
      class="flex items-center gap-2 rounded-md border border-line bg-surface px-3 py-1.5 text-sm font-medium hover:bg-surface-2 disabled:opacity-40"
      disabled={explore.status === 'loading'}
      onclick={() => explore.refresh()}
    >
      <Icon name="repeat" size={16} />Refresh
    </button>
  </div>

  <h2 class="mb-3 mt-6 text-[13px] font-semibold uppercase tracking-wider text-muted">Start from a mood</h2>
  <ul class="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4" aria-label="Moods">
    {#each MOODS as m (m.id)}
      <li>
        <button
          class="flex h-14 w-full items-center rounded-md border border-line border-l-[3px] border-l-accent bg-surface px-4 text-left text-[15px] font-semibold hover:bg-surface-2"
          aria-label="Search {m.label}"
          onclick={() => mood(m.query)}>{m.label}</button>
      </li>
    {/each}
  </ul>

  {#if explore.jump.length > 0}
    <h2 class="mb-3 mt-8 text-[13px] font-semibold uppercase tracking-wider text-muted">Jump back in</h2>
    <ul class="flex gap-4 overflow-x-auto pb-2" aria-label="Jump back in">
      {#each explore.jump as t, i (t.key)}
        <li class="w-36 shrink-0">
          <button class="block w-full text-left" aria-label="Play {t.title}" onclick={() => player.playTracks(explore.jump, i)}>
            <Thumb src={t.thumbnail} seed={t.key} size={144} rounded="rounded-sm" />
            <span class="mt-2 block truncate text-[14px] font-semibold">{t.title}</span>
            <span class="block truncate text-[13px] text-muted">{t.artist}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if explore.status === 'loading' && explore.shelves.length === 0}
    <div class="mt-8"><Skeleton rows={4} /></div>
  {/if}

  {#each explore.shelves as shelf (shelf.id)}
    <div class="mb-2 mt-8 flex items-center justify-between gap-3">
      <h2 class="min-w-0 truncate text-lg font-semibold">{shelf.title}</h2>
      {#if shelf.status === 'ready' && shelf.tracks.length > 0}
        <button
          class="shrink-0 rounded-md bg-accent px-3 py-1.5 text-sm font-semibold text-on-accent"
          aria-label="Play {shelf.title}"
          onclick={() => player.playTracks(shelf.tracks, 0)}>Play all</button>
      {/if}
    </div>
    {#if shelf.status === 'loading'}
      <Skeleton rows={3} />
    {:else if shelf.status === 'error'}
      <p class="rounded-md border border-line bg-surface px-4 py-3 text-sm" role="status">
        Couldn't load this shelf. {shelf.error}
        <button class="ml-2 font-semibold underline" onclick={() => explore.refresh()}>Try again</button>
      </p>
    {:else if shelf.tracks.length === 0}
      <p class="text-sm text-muted">Nothing new to suggest here yet.</p>
    {:else}
      <ul class="grid gap-x-6 md:grid-cols-2" aria-label={shelf.title}>
        {#each shelf.tracks as t, i (t.key)}
          <TrackRow track={t} onplay={() => player.playTracks(shelf.tracks, i)} />
        {/each}
      </ul>
    {/if}
  {/each}

  {#if explore.status === 'ready' && explore.noSeeds}
    <p class="mt-8 rounded-md border border-line bg-surface px-4 py-4 text-sm text-muted">
      Like a few songs or play something and Explore will build shelves of similar music here. Until then, the moods above start a search.
    </p>
  {/if}
</section>
