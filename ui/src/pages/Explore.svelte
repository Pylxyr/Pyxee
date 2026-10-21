<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { search } from '$lib/search.svelte';
  import { explore } from '$lib/explore.svelte';
  import { MOODS } from '$lib/explore';
  import Button from '../components/Button.svelte';
  import CoverGrid from '../components/CoverGrid.svelte';
  import Icon from '../components/Icon.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import SectionHead from '../components/SectionHead.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import TrackList from '../components/TrackList.svelte';

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

<ScrollPage>
  {#snippet header()}
    <PageHeader title="Explore">
      {#snippet actions()}
        <Button disabled={explore.status === 'loading'} onclick={() => explore.refresh()}><Icon name="repeat" size={16} />Refresh</Button>
      {/snippet}
    </PageHeader>
  {/snippet}

  <SectionHead>Start from a mood</SectionHead>
  <ul class="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4" aria-label="Moods">
    {#each MOODS as m (m.id)}
      <li>
        <button
          class="t-strong flex h-14 w-full items-center r-md border border-line border-l-[3px] border-l-accent bg-surface px-4 text-left hover:bg-surface-2"
          aria-label="Search {m.label}"
          onclick={() => mood(m.query)}>{m.label}</button>
      </li>
    {/each}
  </ul>

  {#if explore.jump.length > 0}
    <SectionHead count={explore.jump.length}>Jump back in</SectionHead>
    <CoverGrid label="Jump back in" tracks={explore.jump} onplay={(i) => player.playTracks(explore.jump, i)} />
  {/if}

  {#if explore.status === 'loading' && explore.shelves.length === 0}
    <div class="mt-8"><Skeleton rows={4} /></div>
  {/if}

  {#each explore.shelves as shelf (shelf.id)}
    <SectionHead count={shelf.status === 'ready' ? shelf.tracks.length : null}>
      {shelf.title}
      {#snippet actions()}
        {#if shelf.status === 'ready' && shelf.tracks.length > 0}
          <Button variant="primary" size="sm" aria-label="Play {shelf.title}" onclick={() => player.playTracks(shelf.tracks, 0)}>Play all</Button>
        {/if}
      {/snippet}
    </SectionHead>
    {#if shelf.status === 'loading'}
      <Skeleton rows={3} />
    {:else if shelf.status === 'error'}
      <p class="notice" role="status">
        Couldn't load this shelf. {shelf.error}
        <button class="btn-link ml-2" onclick={() => explore.refresh()}>Try again</button>
      </p>
    {:else if shelf.tracks.length === 0}
      <p class="muted">Nothing new to suggest here yet.</p>
    {:else}
      <TrackList label={shelf.title} tracks={shelf.tracks} onplay={(i) => player.playTracks(shelf.tracks, i)} />
    {/if}
  {/each}

  {#if explore.status === 'ready' && explore.noSeeds}
    <p class="notice muted mt-8">
      Like a few songs or play something and Explore will build shelves of similar music here. Until then, the moods above start a search.
    </p>
  {/if}
</ScrollPage>
