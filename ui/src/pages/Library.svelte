<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav, type LibraryTab } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { messageOf } from '$lib/backend';
  import { groupByDay } from '$lib/format';
  import type { HistoryItemDto } from '$lib/bindings/HistoryItemDto';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import Button from '../components/Button.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import SectionHead from '../components/SectionHead.svelte';
  import Segmented from '../components/Segmented.svelte';
  import Thumb from '../components/Thumb.svelte';
  import TrackRow from '../components/TrackRow.svelte';
  import TrackList from '../components/TrackList.svelte';

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
    <PageHeader title="Library">
      <Segmented items={tabs} value={nav.libraryTab} onselect={(id) => (nav.libraryTab = id)} label="Library sections" />
    </PageHeader>
  {/snippet}

  {#if loadError}<p class="accent-text mb-3" role="alert">{loadError}</p>{/if}

  {#if nav.libraryTab === 'liked'}
    {#if liked.length === 0}
      <EmptyState icon="heart" title="No liked songs yet" text="Tap the heart on a song and it will show up here." />
    {:else}
      <div class="mb-2 flex items-center gap-2">
        <Button variant="primary" onclick={() => playAll(false)}>Play all</Button>
        <Button onclick={() => playAll(true)}>Shuffle</Button>
        <span class="muted ml-auto">{songs(liked.length)}</span>
      </div>
      <TrackList label="Liked songs" tracks={liked} onplay={(i) => player.playTracks(liked, i)} />
    {/if}
  {:else if nav.libraryTab === 'playlists'}
    <div class="mb-2">
      <Button variant="primary" onclick={newPlaylist}><Icon name="plus" size={18} />New playlist</Button>
    </div>
    {#if library.playlists.length === 0}
      <EmptyState icon="library" title="No playlists yet" text="Create one here, or choose Add to playlist on any song. You can also save your queue." />
    {:else}
      <TrackList label="Playlists" head={false}>
        {#each library.playlists as p (p.id)}
          <li class="trow">
            <button class="trow-main" onclick={() => nav.openPlaylist(p.id)}>
              <span class="trow-idx" aria-hidden="true"></span>
              <Thumb src={p.thumbnail} seed={`pl:${p.id}`} size={40} rounded="r-sm" />
              <span class="trow-text">
                <span class="trow-title">{p.name}</span>
                <span class="trow-artist">{songs(p.trackCount)}</span>
              </span>
              <span></span>
            </button>
          </li>
        {/each}
      </TrackList>
    {/if}
  {:else if nav.libraryTab === 'history'}
    {#if history.length === 0}
      <EmptyState icon="music" title="No history yet" text="Songs you listen to for a while are listed here." />
    {:else}
      <div class="mb-2 flex items-center">
        <span class="muted">{library.snapshot.historyCount} {library.snapshot.historyCount === 1 ? 'play' : 'plays'}</span>
        <Button class="ml-auto" onclick={clearHistory}>Clear history</Button>
      </div>
      {#each days as g, gi (g.label)}
        <SectionHead>{g.label}</SectionHead>
        <TrackList label="Played {g.label}" head={gi === 0}>
          {#each g.items as h, i (`${h.playedAt}-${h.track.key}-${i}`)}
            <TrackRow track={h.track} onplay={() => player.playTracks([h.track])} />
          {/each}
        </TrackList>
      {/each}
    {/if}
  {:else}
    <EmptyState icon="library" title="Downloads" text="Songs saved for offline listening live here. Downloads arrive in milestone 7." />
  {/if}
</ScrollPage>
