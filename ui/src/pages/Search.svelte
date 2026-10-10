<script lang="ts">
  import { player } from '$lib/player.svelte';
  import { nav } from '$lib/nav.svelte';
  import { search } from '$lib/search.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { formatTime } from '$lib/format';
  import type { SearchFilterDto } from '$lib/bindings/SearchFilterDto';
  import Icon from '../components/Icon.svelte';
  import Thumb from '../components/Thumb.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import EmptyState from '../components/EmptyState.svelte';

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

<section class="px-4 pb-8 sm:px-8">
  {#if !search.query && search.status === 'idle'}
    <EmptyState icon="search" title="Search" text="Find songs, videos, artists and playlists. Type above and press Enter." />
  {:else}
    <h1 class="text-2xl font-bold">Results for “{search.query}”</h1>
    <div class="mt-3 flex flex-wrap gap-2" role="group" aria-label="Filter results">
      {#each search.filters as f (f.filter)}
        <button
          class="rounded-full px-4 py-1.5 text-sm font-medium disabled:opacity-40 {search.filter === f.filter ? 'bg-fg text-bg' : 'bg-surface-2 text-fg'}"
          aria-pressed={search.filter === f.filter}
          disabled={!f.supported}
          title={f.supported ? undefined : 'Not available with the current music source yet'}
          onclick={() => search.setFilter(f.filter)}>{labels[f.filter]}</button>
      {/each}
    </div>

    <div class="mt-4" aria-live="polite">
      {#if search.status === 'loading'}
        <Skeleton rows={8} />
      {:else if search.status === 'error' && search.error}
        <div class="rounded-xl bg-surface-2 p-5" role="alert">
          <p class="font-semibold">{search.error.message}</p>
          {#if hint}<p class="mt-1 text-sm text-muted">{hint}</p>{/if}
          {#if search.error.kind !== 'unsupported'}
            <button class="mt-3 rounded-full bg-fg px-4 py-1.5 text-sm font-semibold text-bg" onclick={() => search.retry()}>Try again</button>
          {/if}
        </div>
      {:else if search.status === 'ready' && search.items.length === 0}
        <EmptyState icon="search" title="No results" text="Nothing matched “{search.query}”. Try different words or another filter." />
      {:else if search.status === 'ready'}
        <ul class="grid gap-x-6 md:grid-cols-2">
          {#each search.items as item, i (i)}
            {#if item.type === 'track'}
              {@const t = item.track}
              <li class="flex items-center rounded-lg hover:bg-surface">
                <button class="flex min-w-0 flex-1 items-center gap-3 p-2 text-left" onclick={() => player.playTracks([t])}>
                  <Thumb src={t.thumbnail} seed={t.key} size={56} />
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-[15px] font-semibold">{t.title}</span>
                    <span class="block truncate text-[13px] text-muted">{t.artist}</span>
                  </span>
                  <span class="text-[13px] tabular-nums text-muted">{formatTime(t.durationMs)}</span>
                </button>
                <button class="p-2 text-muted hover:text-fg" aria-label="More options for {t.title}" onclick={() => (nav.menuTrack = t)}><Icon name="more" /></button>
              </li>
            {:else if item.type === 'artist'}
              <li class="rounded-lg hover:bg-surface">
                <button class="flex w-full items-center gap-3 p-2 text-left" onclick={() => search.run(item.name, 'songs')}>
                  <Thumb src={item.thumbnail} seed={item.key} size={56} rounded="rounded-full" />
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-[15px] font-semibold">{item.name}</span>
                    <span class="block text-[13px] text-muted">Artist · show songs</span>
                  </span>
                </button>
              </li>
            {:else}
              <li class="rounded-lg hover:bg-surface">
                <button class="flex w-full items-center gap-3 p-2 text-left" onclick={() => snackbar.push('Opening playlists arrives with the library (milestone 5).')}>
                  <Thumb src={item.thumbnail} seed={item.key} size={56} />
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-[15px] font-semibold">{item.title}</span>
                    <span class="block truncate text-[13px] text-muted">Playlist · {item.uploader}</span>
                  </span>
                </button>
              </li>
            {/if}
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</section>
