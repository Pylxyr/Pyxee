<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { search } from '$lib/search.svelte';
  import { explore } from '$lib/explore.svelte';
  import { albumQuery } from '$lib/explore';
  import type { AlbumDto } from '$lib/bindings/AlbumDto';
  import AlbumGrid from '../components/AlbumGrid.svelte';
  import Button from '../components/Button.svelte';
  import CoverGrid from '../components/CoverGrid.svelte';
  import Icon from '../components/Icon.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import SectionHead from '../components/SectionHead.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import TrackList from '../components/TrackList.svelte';

  // Loaded once per session (every shelf is a request to YouTube); Refresh asks again.
  $effect(() => {
    const backend = library.backend;
    if (backend && library.ready) void explore.ensure(backend);
  });

  // There is no album page yet: a cover opens an ordinary songs search for the release.
  function openAlbum(album: AlbumDto) {
    nav.go('search');
    void search.run(albumQuery(album), 'songs');
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

  {#if explore.jump.length > 0}
    <SectionHead count={explore.jump.length}>Jump back in</SectionHead>
    <CoverGrid label="Jump back in" tracks={explore.jump} onplay={(i) => player.playTracks(explore.jump, i)} />
  {/if}

  {#if explore.browseStatus === 'loading'}
    <div class="mt-8"><Skeleton rows={4} /></div>
  {:else if explore.browseStatus === 'error'}
    <p class="notice mt-8" role="status">
      Couldn't load YouTube Music's picks. {explore.browseError}
      <button class="btn-link ml-2" onclick={() => explore.refresh()}>Try again</button>
    </p>
  {/if}

  {#each explore.browse as shelf (shelf.id)}
    {#if shelf.kind === 'songs'}
      <SectionHead count={shelf.tracks.length}>
        {shelf.title}
        {#snippet actions()}
          <Button variant="primary" size="sm" aria-label="Play {shelf.title}" onclick={() => player.playTracks(shelf.tracks, 0)}>Play all</Button>
        {/snippet}
      </SectionHead>
      <CoverGrid label={shelf.title} tracks={shelf.tracks} onplay={(i) => player.playTracks(shelf.tracks, i)} />
    {:else}
      <SectionHead count={shelf.albums.length}>{shelf.title}</SectionHead>
      <AlbumGrid label={shelf.title} albums={shelf.albums} onopen={openAlbum} />
    {/if}
  {/each}

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
      Like a few songs or play something and Explore will add shelves of similar music here.
    </p>
  {/if}
</ScrollPage>
