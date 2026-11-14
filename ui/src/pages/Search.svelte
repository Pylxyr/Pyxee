<script lang="ts">
  import { player } from '$lib/player.svelte';
  import { search } from '$lib/search.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
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

  const labels: Record<SearchFilterDto, string> = {
    all: 'All', songs: 'Songs', videos: 'Videos', albums: 'Albums', artists: 'Artists', playlists: 'Playlists',
  };
  const hint = $derived(
    search.error?.kind === 'offline'
      ? 'Check your internet connection, then try again.'
      : search.error?.kind === 'toolMissing'
        ? 'Pyxee looks for yt-dlp next to the app. Run "cargo xtask tools" (see docs/development.md) or set PYXEE_YTDLP to its path.'
        : search.error?.kind === 'rateLimited'
          ? 'Wait a little while before searching again.'
          : '',
  );
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
      <PageHeader title="Results for “{search.query}”">
        <Segmented mode="toggle" items={filters} value={search.filter} onselect={(f) => search.setFilter(f)} label="Filter results" />
      </PageHeader>
    {/if}
  {/snippet}

  {#if !search.query && search.status === 'idle'}
    <EmptyState icon="search" title="Search" text="Find songs, videos, artists and playlists. Type above and press Enter." />
  {:else}
    <div aria-live="polite">
      {#if search.status === 'loading'}
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
      {:else if search.status === 'ready'}
        <TrackList label="Results" head={false}>
          {#each search.items as item, i (i)}
            {#if item.type === 'track'}
              {@const t = item.track}
              <TrackRow track={t} index={i} onplay={() => player.playTracks([t])} />
            {:else if item.type === 'artist'}
              <li class="trow">
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
              <li class="trow">
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
      {/if}
    </div>
  {/if}
</ScrollPage>
