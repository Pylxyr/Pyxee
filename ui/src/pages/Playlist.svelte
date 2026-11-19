<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { messageOf } from '$lib/backend';
  import { Loaded } from '$lib/loaded.svelte';
  import { reorder } from '$lib/reorder';
  import { focusNeighbour } from '$lib/focus';
  import { markListChange } from '$lib/motion';
  import { flipList } from '$lib/flip';
  import type { PlaylistDto } from '$lib/bindings/PlaylistDto';
  import Button from '../components/Button.svelte';
  import EditableRow from '../components/EditableRow.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import IconButton from '../components/IconButton.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import Thumb from '../components/Thumb.svelte';

  // The playlist loads on its own (`Loaded`): "not there" is shown only when the backend says it is gone, a failed read
  // keeps what is on screen, and opening another playlist starts from a clean slate.
  const source = new Loaded<PlaylistDto>();
  const pl = $derived(source.data);
  /** The row being dragged and where it would land (null when nothing is being dragged). */
  let drag = $state<{ id: number; from: number; to: number } | null>(null);
  let listEl = $state<HTMLElement>();
  const rows = () => [...(listEl?.querySelectorAll<HTMLElement>('[data-row]') ?? [])];
  let shownId: number | null = null;

  $effect(() => {
    void library.revision;
    const id = nav.playlistId;
    const b = library.backend;
    if (id == null || !b) return;
    if (shownId !== id) {
      shownId = id;
      source.reset();
    }
    void source.load(() => b.playlistGet(id));
  });

  const tracks = $derived(pl?.entries.map((e) => e.track) ?? []);

  function play(shuffle: boolean, start = 0) {
    if (tracks.length === 0) return;
    if (shuffle !== player.state.shuffle) player.toggleShuffle();
    player.playTracks(tracks, start);
  }

  function rename() {
    const p = pl;
    if (!p) return;
    nav.namePrompt = {
      title: 'Rename playlist',
      initial: p.name,
      confirm: 'Save',
      run: (name) => library.backend!.playlistRename(p.id, name).catch((e) => Promise.reject(messageOf(e))),
    };
  }

  async function remove() {
    const p = pl;
    if (!p) return;
    try {
      const snap = await library.backend!.playlistDelete(p.id);
      nav.libraryTab = 'playlists';
      nav.go('library');
      snackbar.push(`Deleted "${p.name}"`, { action: { label: 'Undo', run: () => void library.backend!.playlistRestore(snap) } });
    } catch (e) {
      snackbar.push(messageOf(e));
    }
  }

  async function removeEntry(entryId: number, title: string, from: Element | null) {
    markListChange();
    focusNeighbour(from);
    try {
      const removed = await library.backend!.playlistRemoveEntry(entryId);
      snackbar.push(`Removed "${title}"`, { action: { label: 'Undo', run: () => void library.backend!.playlistRestoreEntry(removed) } });
    } catch (e) {
      snackbar.push(messageOf(e));
    }
  }

  function move(entryId: number, to: number) {
    if (!pl || to < 0 || to >= pl.entries.length) return;
    library.backend!.playlistMoveEntry(entryId, to).catch((e) => snackbar.push(messageOf(e)));
  }
  const overOf = (id: number, i: number): 'above' | 'below' | null =>
    drag && drag.to === i && drag.id !== id ? (drag.to < drag.from ? 'above' : 'below') : null;
</script>

<ScrollPage>
  {#snippet header()}
    {#if pl}
      <PageHeader title={pl.name} meta="{pl.entries.length} {pl.entries.length === 1 ? 'song' : 'songs'}">
        {#snippet lead()}
          <Thumb src={pl?.entries[0]?.track.thumbnail ?? null} seed={`pl:${pl?.id}`} size={160} rounded="r-sm" />
        {/snippet}
        {#snippet actions()}
          <Button variant="primary" disabled={tracks.length === 0} onclick={() => play(false)}>Play</Button>
          <Button disabled={tracks.length === 0} onclick={() => play(true)}>Shuffle</Button>
          <Button onclick={rename}>Rename</Button>
          <Button onclick={remove}>Delete</Button>
        {/snippet}
      </PageHeader>
    {:else if !source.missing}
      <PageHeader title="Playlist" />
    {/if}
  {/snippet}

  {#if source.missing}
    <EmptyState icon="library" title="This playlist no longer exists" text="It may have been deleted. Your other playlists are in the Library." />
  {:else if !pl && source.error}
    <div class="notice" role="alert">
      <p class="t-strong">The playlist could not be opened.</p>
      <p class="muted mt-1">{source.error}</p>
      <Button class="mt-3" variant="primary" onclick={() => nav.playlistId != null && library.backend && void source.load(() => library.backend!.playlistGet(nav.playlistId!))}>Try again</Button>
    </div>
  {:else if !pl}
    <Skeleton rows={8} />
  {:else}
    {#if source.error}<p class="accent-text mb-3" role="alert">Couldn't refresh this playlist: {source.error}</p>{/if}
    {#if pl.entries.length === 0}
      <EmptyState icon="music" title="This playlist is empty" text="Choose Add to playlist on any song to put it here." />
    {:else}
      {@const list = pl}
      <ul aria-label="Songs in {list.name}" bind:this={listEl} use:flipList>
        {#each list.entries as e, i (e.entryId)}
          <EditableRow
            data-row
            track={e.track}
            index={i}
            over={overOf(e.entryId, i)}
            dragging={drag?.id === e.entryId}
            onplay={() => play(false, i)}
          >
            {#snippet handle()}
              <span
                class="handle"
                role="presentation"
                title="Drag to reorder"
                data-testid="drag-handle"
                use:reorder={{
                  index: i,
                  rows,
                  onover: (to) => (drag = to === null ? null : { id: e.entryId, from: i, to }),
                  ondrop: (to) => move(e.entryId, to),
                }}><Icon name="drag" size={18} /></span>
            {/snippet}
            {#snippet actions()}
              <IconButton icon="up" iconSize={18} label="Move {e.track.title} up" disabled={i === 0} onclick={() => move(e.entryId, i - 1)} />
              <IconButton icon="down" iconSize={18} label="Move {e.track.title} down" disabled={i === list.entries.length - 1} onclick={() => move(e.entryId, i + 1)} />
              <IconButton icon="close" iconSize={18} label="Remove {e.track.title} from playlist" onclick={(ev) => removeEntry(e.entryId, e.track.title, ev.currentTarget)} />
            {/snippet}
          </EditableRow>
        {/each}
      </ul>
    {/if}
  {/if}
</ScrollPage>
