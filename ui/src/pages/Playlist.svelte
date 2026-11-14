<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { messageOf } from '$lib/backend';
  import type { PlaylistDto } from '$lib/bindings/PlaylistDto';
  import Button from '../components/Button.svelte';
  import EditableRow from '../components/EditableRow.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import IconButton from '../components/IconButton.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import Thumb from '../components/Thumb.svelte';

  let pl = $state<PlaylistDto | null>(null);
  let missing = $state(false);
  let dragId = $state<number | null>(null);
  let overIndex = $state<number | null>(null);

  $effect(() => {
    void library.revision;
    const id = nav.playlistId;
    const b = library.backend;
    if (id == null || !b) return;
    b.playlistGet(id).then((p) => ((pl = p), (missing = false))).catch(() => ((pl = null), (missing = true)));
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

  async function removeEntry(entryId: number, title: string) {
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
  function drop(e: DragEvent, to: number) {
    e.preventDefault();
    if (dragId != null) move(dragId, to);
    dragId = overIndex = null;
  }
</script>

<ScrollPage>
  {#snippet header()}
    {#if pl && !missing}
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
    {/if}
  {/snippet}

  {#if missing || !pl}
    <EmptyState icon="library" title="This playlist no longer exists" text="It may have been deleted. Your other playlists are in the Library." />
  {:else if pl.entries.length === 0}
    <EmptyState icon="music" title="This playlist is empty" text="Choose Add to playlist on any song to put it here." />
  {:else}
    {@const list = pl}
    <ul aria-label="Songs in {list.name}">
      {#each list.entries as e, i (e.entryId)}
        <EditableRow
          track={e.track}
          index={i}
          over={overIndex === i && dragId !== e.entryId ? 'above' : null}
          dragging={dragId === e.entryId}
          onplay={() => play(false, i)}
          draggable={true}
          ondragstart={(ev) => {
            dragId = e.entryId;
            ev.dataTransfer?.setData('text/plain', String(e.entryId));
            if (ev.dataTransfer) ev.dataTransfer.effectAllowed = 'move';
          }}
          ondragover={(ev) => {
            ev.preventDefault();
            overIndex = i;
          }}
          ondrop={(ev) => drop(ev, i)}
          ondragend={() => (dragId = overIndex = null)}
        >
          {#snippet handle()}
            <span class="handle" aria-hidden="true"><Icon name="drag" size={18} /></span>
          {/snippet}
          {#snippet actions()}
            <IconButton icon="up" iconSize={18} label="Move {e.track.title} up" disabled={i === 0} onclick={() => move(e.entryId, i - 1)} />
            <IconButton icon="down" iconSize={18} label="Move {e.track.title} down" disabled={i === list.entries.length - 1} onclick={() => move(e.entryId, i + 1)} />
            <IconButton icon="close" iconSize={18} label="Remove {e.track.title} from playlist" onclick={() => removeEntry(e.entryId, e.track.title)} />
          {/snippet}
        </EditableRow>
      {/each}
    </ul>
  {/if}
</ScrollPage>
