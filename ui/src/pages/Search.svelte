<script lang="ts">
  import { player } from '$lib/player.svelte';
  import { search } from '$lib/search.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { messageOf } from '$lib/backend';
  import { reveal } from '$lib/motion';
  import type { SearchFilterDto } from '$lib/bindings/SearchFilterDto';
  import Button from '../components/Button.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import Segmented from '../components/Segmented.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import Thumb from '../components/Thumb.svelte';
  import TrackList from '../components/TrackList.svelte';
  import TrackRow from '../components/TrackRow.svelte';
  import { withKeys } from '$lib/keys';

  const labels: Record<SearchFilterDto, string> = {
    all: 'All', songs: 'Songs', videos: 'Videos', albums: 'Albums', artists: 'Artists', playlists: 'Playlists',
  };
  const hint = $derived(
    search.error?.kind === 'offline'
      ? 'Check your internet connection, then try again.'
      : search.error?.kind === 'toolMissing'
        ? __DEV_TOOLS__
          ? 'Pyxee looks for yt-dlp next to the app. Run "cargo xtask tools" (see docs/development.md) or set PYXEE_YTDLP to its path.'
          : 'Pyxee could not find its copy of yt-dlp. Open Settings, then Updates, and update yt-dlp; if that does not help, reinstall Pyxee.'
        : search.error?.kind === 'rateLimited'
          ? 'Wait a little while before searching again.'
          : '',
  );
  // The one short sentence a screen reader hears (the list itself is not a live region: it would read every row).
  const status = $derived(
    search.status === 'loading'
      ? 'Searching'
      : search.status === 'error'
        ? 'The search failed'
        : search.status === 'ready'
          ? search.items.length === 0
            ? 'Nothing found'
            : `${search.items.length} ${search.items.length === 1 ? 'result' : 'results'}`
          : '',
  );
  // Results of the same query stay up, dimmed, while another filter loads.
  const showList = $derived(search.status === 'ready' || (search.status === 'loading' && search.reloading));
  const rows = $derived(withKeys(search.items, (i) => (i.type === 'track' ? i.track.key : `${i.type}:${i.key}`)));
  /** The songs of a playlist link, in the order YouTube lists them. */
  const listTracks = $derived(search.items.flatMap((i) => (i.type === 'track' ? [i.track] : [])));

  function playList(shuffle: boolean, start = 0) {
    if (listTracks.length === 0) return;
    if (shuffle !== player.state.shuffle) player.toggleShuffle();
    player.playTracks(listTracks, start);
  }

  function saveList() {
    const tracks = listTracks;
    nav.namePrompt = {
      title: 'Save as playlist',
      initial: '',
      confirm: 'Save',
      run: async (name) => {
        const id = await library.backend!.playlistCreate(name, tracks).catch((e) => Promise.reject(messageOf(e)));
        snackbar.push(`Saved ${tracks.length} ${tracks.length === 1 ? 'song' : 'songs'} as ${name.trim()}`, { action: { label: 'Open', run: () => nav.openPlaylist(id) } });
      },
    };
  }

  const filters = $derived(
    search.filters.map((f) => ({
      id: f.filter,
      label: labels[f.filter],
      disabled: !f.supported,
      title: f.supported ? undefined : 'Not available with the current music source yet',
    })),
  );
</script>

<ScrollPage>
  {#snippet header()}
    {#if search.query || search.status !== 'idle'}
      <PageHeader title="Results for “{search.query}”" inline>
        <Segmented mode="toggle" items={filters} value={search.filter} onselect={(f) => search.setFilter(f)} label="Filter results" />
      </PageHeader>
    {/if}
  {/snippet}

  {#if !search.query && search.status === 'idle'}
    <EmptyState icon="search" title="Search" text="Find songs, videos, artists and playlists. Type above and press Enter." />
  {:else}
    <p class="sr-only" role="status">{status}</p>
    <div aria-busy={search.status === 'loading'}>
      {#if search.status === 'loading' && !search.reloading}
        <Skeleton rows={8} />
      {:else if search.status === 'error' && search.error}
        <div class="notice" role="alert">
          <p class="t-strong">{search.error.message}</p>
          {#if hint}<p class="muted mt-1">{hint}</p>{/if}
          {#if search.error.kind !== 'unsupported'}
            <Button class="mt-3" variant="primary" onclick={() => search.retry()}>Try again</Button>
          {/if}
        </div>
      {:else if search.status === 'ready' && search.items.length === 0}
        <EmptyState icon="search" title="No results" text="Nothing matched “{search.query}”. Try different words or another filter." />
      {:else if showList}
        <!-- route-in: a new set of results (after the skeleton) arrives with the container. Each row (and its cover) reveals itself as it scrolls into view. A filter change keeps this element, so its dimming stays smooth. -->
        <div class="results route-in" data-dim={search.status === 'loading'}>
        {#if search.listLink}
          <div class="toolbar">
            <Button variant="primary" onclick={() => playList(false)}>Play all</Button>
            <Button onclick={() => playList(true)}>Shuffle</Button>
            <Button onclick={saveList}>Save as playlist</Button>
            <span class="muted ml-auto">{listTracks.length} {listTracks.length === 1 ? 'song' : 'songs'}</span>
          </div>
        {/if}
        <TrackList label="Results" head={false}>
          {#each rows as r (r.key)}
            {@const item = r.item}
            {@const i = r.index}
            {#if item.type === 'track'}
              {@const t = item.track}
              <TrackRow track={t} index={i} onplay={() => (search.listLink ? playList(false, i) : player.playTracks([t]))} />
            {:else if item.type === 'artist'}
              <li class="trow" use:reveal>
                <button class="trow-main" onclick={() => search.run(item.name, 'songs')}>
                  <span class="trow-idx" aria-hidden="true"></span>
                  <Thumb src={item.thumbnail} seed={item.key} size={40} rounded="r-full" />
                  <span class="trow-text">
                    <span class="trow-title">{item.name}</span>
                    <span class="trow-artist">Artist · show songs</span>
                  </span>
                  <span></span>
                </button>
              </li>
            {:else}
              <li class="trow" use:reveal>
                <button class="trow-main" onclick={() => snackbar.push('Opening playlists from search is not available yet.')}>
                  <span class="trow-idx" aria-hidden="true"></span>
                  <Thumb src={item.thumbnail} seed={item.key} size={40} rounded="r-sm" />
                  <span class="trow-text">
                    <span class="trow-title">{item.title}</span>
                    <span class="trow-artist">Playlist · {item.uploader}</span>
                  </span>
                  <span></span>
                </button>
              </li>
            {/if}
          {/each}
        </TrackList>
        </div>
      {/if}
    </div>
  {/if}
</ScrollPage>
