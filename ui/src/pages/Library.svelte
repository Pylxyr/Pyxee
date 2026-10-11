<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav, type LibraryTab } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { messageOf } from '$lib/backend';
  import { groupByDay } from '$lib/format';
  import type { HistoryItemDto } from '$lib/bindings/HistoryItemDto';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import EmptyState from '../components/EmptyState.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import Icon from '../components/Icon.svelte';
  import Thumb from '../components/Thumb.svelte';
  import TrackRow from '../components/TrackRow.svelte';

  const tabs: { id: LibraryTab; label: string }[] = [
    { id: 'liked', label: 'Liked songs' },
    { id: 'playlists', label: 'Playlists' },
    { id: 'history', label: 'History' },
    { id: 'downloads', label: 'Downloads' },
  ];

  let liked = $state<TrackDto[]>([]);
  let history = $state<HistoryItemDto[]>([]);
  let loadError = $state('');

  // Detail lists reload whenever the library changes (likes, plays, undo, ...).
  $effect(() => {
    void library.revision;
    const b = library.backend;
    if (!b) return;
    if (nav.libraryTab === 'liked') b.likedGet().then((l) => ((liked = l), (loadError = ''))).catch((e) => (loadError = messageOf(e)));
    if (nav.libraryTab === 'history') b.historyGet(300).then((h) => ((history = h), (loadError = ''))).catch((e) => (loadError = messageOf(e)));
  });

  const days = $derived(groupByDay(history, (h) => h.playedAt, Date.now()));

  function newPlaylist() {
    nav.namePrompt = {
      title: 'New playlist',
      initial: '',
      confirm: 'Create',
      run: async (name) => {
        const id = await library.backend!.playlistCreate(name, []).catch((e) => Promise.reject(messageOf(e)));
        nav.openPlaylist(id);
      },
    };
  }

  async function clearHistory() {
    try {
      const items = await library.backend!.historyClear();
      snackbar.push('History cleared', { action: { label: 'Undo', run: () => void library.backend!.historyRestore(items) } });
    } catch (e) {
      snackbar.push(messageOf(e));
    }
  }

  function playAll(shuffle: boolean) {
    if (liked.length === 0) return;
    if (shuffle !== player.state.shuffle) player.toggleShuffle();
    player.playTracks(liked, 0);
  }
</script>

<ScrollPage>
  {#snippet header()}
  <h1 class="text-2xl font-bold">Library</h1>
  <div class="mt-4 flex flex-wrap gap-2" role="tablist" aria-label="Library sections">
    {#each tabs as t (t.id)}
      <button
        role="tab"
        aria-selected={nav.libraryTab === t.id}
        class="rounded-full px-4 py-1.5 text-sm font-medium {nav.libraryTab === t.id ? 'bg-fg text-on-fg' : 'bg-surface-2 text-fg'}"
        onclick={() => (nav.libraryTab = t.id)}>{t.label}</button>
    {/each}
  </div>
  {/snippet}

  {#if loadError}<p class="mt-4 text-sm text-accent" role="alert">{loadError}</p>{/if}

  {#if nav.libraryTab === 'liked'}
    {#if liked.length === 0}
      <EmptyState icon="heart" title="No liked songs yet" text="Tap the heart on a song and it will show up here." />
    {:else}
      <div class="mt-4 flex gap-2">
        <button class="rounded-full bg-fg px-5 py-2 text-sm font-semibold text-on-fg" onclick={() => playAll(false)}>Play all</button>
        <button class="rounded-full bg-surface-2 px-5 py-2 text-sm font-semibold" onclick={() => playAll(true)}>Shuffle</button>
        <span class="ml-auto self-center text-sm text-muted">{liked.length} {liked.length === 1 ? 'song' : 'songs'}</span>
      </div>
      <ul class="mt-2 grid gap-x-6 md:grid-cols-2" aria-label="Liked songs">
        {#each liked as t, i (t.key)}
          <TrackRow track={t} onplay={() => player.playTracks(liked, i)} />
        {/each}
      </ul>
    {/if}
  {:else if nav.libraryTab === 'playlists'}
    <div class="mt-4">
      <button class="flex items-center gap-2 rounded-full bg-fg px-5 py-2 text-sm font-semibold text-on-fg" onclick={newPlaylist}><Icon name="plus" size={18} />New playlist</button>
    </div>
    {#if library.playlists.length === 0}
      <EmptyState icon="library" title="No playlists yet" text="Create one here, or choose Add to playlist on any song. You can also save your queue." />
    {:else}
      <ul class="mt-4 grid gap-x-6 md:grid-cols-2" aria-label="Playlists">
        {#each library.playlists as p (p.id)}
          <li class="rounded-lg hover:bg-surface">
            <button class="flex w-full items-center gap-3 p-2 text-left" onclick={() => nav.openPlaylist(p.id)}>
              <Thumb src={p.thumbnail} seed={`pl:${p.id}`} size={56} />
              <span class="min-w-0 flex-1">
                <span class="block truncate text-[15px] font-semibold">{p.name}</span>
                <span class="block text-[13px] text-muted">{p.trackCount} {p.trackCount === 1 ? 'song' : 'songs'}</span>
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {:else if nav.libraryTab === 'history'}
    {#if history.length === 0}
      <EmptyState icon="music" title="No history yet" text="Songs you listen to for a while are listed here." />
    {:else}
      <div class="mt-4 flex items-center">
        <span class="text-sm text-muted">{library.snapshot.historyCount} {library.snapshot.historyCount === 1 ? 'play' : 'plays'}</span>
        <button class="ml-auto rounded-full bg-surface-2 px-4 py-1.5 text-sm font-medium" onclick={clearHistory}>Clear history</button>
      </div>
      {#each days as g (g.label)}
        <h2 class="mb-1 mt-5 text-sm font-semibold text-muted">{g.label}</h2>
        <ul class="grid gap-x-6 md:grid-cols-2" aria-label="Played {g.label}">
          {#each g.items as h, i (`${h.playedAt}-${h.track.key}-${i}`)}
            <TrackRow track={h.track} onplay={() => player.playTracks([h.track])} />
          {/each}
        </ul>
      {/each}
    {/if}
  {:else}
    <EmptyState icon="library" title="Downloads" text="Songs saved for offline listening live here. Downloads arrive in milestone 7." />
  {/if}
</ScrollPage>
