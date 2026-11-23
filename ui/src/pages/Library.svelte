<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav, type LibraryTab } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { messageOf } from '$lib/backend';
  import { groupByDay } from '$lib/format';
  import { Loaded } from '$lib/loaded.svelte';
  import { reveal } from '$lib/motion';
  import { withKeys } from '$lib/keys';
  import type { HistoryItemDto } from '$lib/bindings/HistoryItemDto';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import Button from '../components/Button.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import SectionHead from '../components/SectionHead.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import Segmented from '../components/Segmented.svelte';
  import Thumb from '../components/Thumb.svelte';
  import TrackRow from '../components/TrackRow.svelte';
  import TrackList from '../components/TrackList.svelte';
  import LocalMusic from '../components/LocalMusic.svelte';
  import OfflineCache from '../components/OfflineCache.svelte';

  const tabs: { id: LibraryTab; label: string }[] = [
    { id: 'liked', label: 'Liked songs' },
    { id: 'playlists', label: 'Playlists' },
    { id: 'history', label: 'History' },
    { id: 'device', label: 'On this device' },
    { id: 'downloads', label: 'Offline' },
  ];

  // Each tab loads on its own: its own "arrived yet", its own error, and a read that finishes late never replaces a
  // newer one. A failed read keeps what the tab already shows.
  const likedSource = new Loaded<TrackDto[]>();
  const historySource = new Loaded<HistoryItemDto[]>();
  const liked = $derived(likedSource.data ?? []);
  const history = $derived(historySource.data ?? []);

  // Detail lists reload whenever the library changes (likes, plays, undo, ...).
  $effect(() => {
    void library.revision;
    const b = library.backend;
    if (!b) return;
    if (nav.libraryTab === 'liked') void likedSource.load(() => b.likedGet());
    if (nav.libraryTab === 'history') void historySource.load(() => b.historyGet(300));
  });

  const days = $derived(groupByDay(history, (h) => h.playedAt, Date.now()));
  const songs = (n: number) => `${n} ${n === 1 ? 'song' : 'songs'}`;

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
    <PageHeader title="Library" inline>
      <Segmented items={tabs} value={nav.libraryTab} onselect={(id) => (nav.libraryTab = id)} label="Library sections" />
    </PageHeader>
  {/snippet}

  {#if nav.libraryTab === 'liked'}
    {#if likedSource.error}<p class="accent-text mb-3" role="alert">{likedSource.error}</p>{/if}
    {#if !likedSource.loaded && !likedSource.error}
      <Skeleton rows={8} />
    {:else if liked.length === 0}
      <EmptyState icon="heart" title="No liked songs yet" text="Tap the heart on a song and it will show up here." />
    {:else}
      <div class="toolbar">
        <Button variant="primary" onclick={() => playAll(false)}>Play all</Button>
        <Button onclick={() => playAll(true)}>Shuffle</Button>
        <span class="muted ml-auto">{songs(liked.length)}</span>
      </div>
      <TrackList label="Liked songs" tracks={liked} onplay={(i) => player.playTracks(liked, i)} />
    {/if}
  {:else if nav.libraryTab === 'playlists'}
    <div class="toolbar">
      <Button variant="primary" onclick={newPlaylist}><Icon name="plus" size={18} />New playlist</Button>
    </div>
    {#if library.playlists.length === 0}
      <EmptyState icon="library" title="No playlists yet" text="Create one here, or choose Add to playlist on any song. You can also save your queue." />
    {:else}
      <ul class="cover-grid mt-4" aria-label="Playlists">
        {#each library.playlists as p (p.id)}
          <li use:reveal>
            <button class="cover-tile" onclick={() => nav.openPlaylist(p.id)}>
              <span class="cover-art block"><Thumb src={p.thumbnail} seed={`pl:${p.id}`} size={160} fill rounded="r-sm" /></span>
              <span class="trow-title mt-2 block">{p.name}</span>
              <span class="trow-artist block">{songs(p.trackCount)}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {:else if nav.libraryTab === 'history'}
    {#if historySource.error}<p class="accent-text mb-3" role="alert">{historySource.error}</p>{/if}
    {#if !historySource.loaded && !historySource.error}
      <Skeleton rows={8} />
    {:else if history.length === 0}
      <EmptyState icon="music" title="No history yet" text="Songs you listen to for a while are listed here." />
    {:else}
      <div class="toolbar">
        <span class="muted">{library.snapshot.historyCount} {library.snapshot.historyCount === 1 ? 'play' : 'plays'}</span>
        <Button class="ml-auto" onclick={clearHistory}>Clear history</Button>
      </div>
      {#each days as g, gi (g.label)}
        <SectionHead>{g.label}</SectionHead>
        <TrackList label="Played {g.label}" head={gi === 0}>
          {#each withKeys(g.items, (h) => `${h.playedAt}-${h.track.key}`) as r (r.key)}
            <TrackRow track={r.item.track} onplay={() => player.playTracks([r.item.track])} />
          {/each}
        </TrackList>
      {/each}
    {/if}
  {:else if nav.libraryTab === 'device'}
    <LocalMusic />
  {:else}
    <OfflineCache />
  {/if}
</ScrollPage>
