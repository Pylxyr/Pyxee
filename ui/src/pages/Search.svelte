<script lang="ts">
  import { player } from '$lib/player.svelte';
  import { nav } from '$lib/nav.svelte';
  import { search } from '$lib/search.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import type { SearchFilterDto } from '$lib/bindings/SearchFilterDto';
  import Icon from '../components/Icon.svelte';
  import Thumb from '../components/Thumb.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import SectionHead from '../components/SectionHead.svelte';
  import TrackRow from '../components/TrackRow.svelte';

  const labels: Record<SearchFilterDto, string> = {
    all: 'All', songs: 'Songs', videos: 'Videos', albums: 'Albums', artists: 'Artists', playlists: 'Playlists',
  };
  const hint = $derived(
    search.error?.kind === 'offline'
      ? 'Check your internet connection, then try again.'
      : search.error?.kind === 'toolMissing'
        ? 'Pyxee looks for yt-dlp next to the app. Run "cargo xtask tools" (see the README) or set PYXEE_YTDLP to its path.'
        : search.error?.kind === 'rateLimited'
          ? 'Wait a little while before searching again.'
          : '',
  );
</script>

<ScrollPage>
  {#snippet header()}
    {#if search.query || search.status !== 'idle'}
      <SectionHead level={1}>Results for “{search.query}”</SectionHead>
      <div class="flex flex-wrap gap-x-5 border-b border-line" role="group" aria-label="Filter results">
        {#each search.filters as f (f.filter)}
          <button
            class="-mb-px border-b-2 px-0.5 py-2 text-sm font-medium disabled:opacity-40 {search.filter === f.filter ? 'border-accent text-fg' : 'border-transparent text-muted hover:text-fg'}"
            aria-pressed={search.filter === f.filter}
            disabled={!f.supported}
            title={f.supported ? undefined : 'Not available with the current music source yet'}
            onclick={() => search.setFilter(f.filter)}>{labels[f.filter]}</button>
        {/each}
      </div>
    {/if}
  {/snippet}

  {#if !search.query && search.status === 'idle'}
    <EmptyState icon="search" title="Search" text="Find songs, videos, artists and playlists. Type above and press Enter." />
  {:else}
    <div aria-live="polite">
      {#if search.status === 'loading'}
        <Skeleton rows={8} />
      {:else if search.status === 'error' && search.error}
        <div class="rounded border border-line bg-surface p-5" role="alert">
          <p class="font-semibold">{search.error.message}</p>
          {#if hint}<p class="mt-1 text-sm text-muted">{hint}</p>{/if}
          {#if search.error.kind !== 'unsupported'}
            <button class="mt-3 rounded-md bg-fg px-4 py-1.5 text-sm font-semibold text-on-fg" onclick={() => search.retry()}>Try again</button>
          {/if}
        </div>
      {:else if search.status === 'ready' && search.items.length === 0}
        <EmptyState icon="search" title="No results" text="Nothing matched “{search.query}”. Try different words or another filter." />
      {:else if search.status === 'ready'}
        <ul>
          {#each search.items as item, i (i)}
            {#if item.type === 'track'}
              {@const t = item.track}
              <TrackRow track={t} index={i} onplay={() => player.playTracks([t])} />
            {:else if item.type === 'artist'}
              <li class="border-b border-line/60 hover:bg-surface-2/70">
                <button class="flex w-full items-center gap-3 py-1.5 pl-2 text-left" onclick={() => search.run(item.name, 'songs')}>
                  <span class="w-6 shrink-0" aria-hidden="true"></span>
                  <Thumb src={item.thumbnail} seed={item.key} size={40} rounded="rounded-full" />
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-[14px] font-medium">{item.name}</span>
                    <span class="block text-[12px] text-muted">Artist · show songs</span>
                  </span>
                </button>
              </li>
            {:else}
              <li class="border-b border-line/60 hover:bg-surface-2/70">
                <button class="flex w-full items-center gap-3 py-1.5 pl-2 text-left" onclick={() => snackbar.push('Opening playlists from search is not available yet.')}>
                  <span class="w-6 shrink-0" aria-hidden="true"></span>
                  <Thumb src={item.thumbnail} seed={item.key} size={40} rounded="rounded-sm" />
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-[14px] font-medium">{item.title}</span>
                    <span class="block truncate text-[12px] text-muted">Playlist · {item.uploader}</span>
                  </span>
                </button>
              </li>
            {/if}
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</ScrollPage>
