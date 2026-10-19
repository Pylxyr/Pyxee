<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { messageOf } from '$lib/backend';
  import { formatTime } from '$lib/format';
  import type { PlaylistDto } from '$lib/bindings/PlaylistDto';
  import EmptyState from '../components/EmptyState.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import Icon from '../components/Icon.svelte';
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
      <div class="flex flex-wrap items-center gap-4">
        <Thumb src={pl.entries[0]?.track.thumbnail ?? null} seed={`pl:${pl.id}`} size={96} rounded="rounded-sm" />
        <div class="min-w-0 flex-1">
          <h1 class="truncate text-2xl font-semibold tracking-tight">{pl.name}</h1>
          <p class="text-sm text-muted">{pl.entries.length} {pl.entries.length === 1 ? 'song' : 'songs'}</p>
        </div>
      </div>
      <div class="mt-4 flex flex-wrap gap-2">
        <button class="rounded bg-accent px-5 py-2 text-sm font-semibold text-on-accent disabled:opacity-40" disabled={tracks.length === 0} onclick={() => play(false)}>Play</button>
        <button class="rounded-md border border-line bg-surface px-5 py-2 text-sm font-semibold disabled:opacity-40" disabled={tracks.length === 0} onclick={() => play(true)}>Shuffle</button>
        <button class="rounded-md border border-line bg-surface px-5 py-2 text-sm font-medium" onclick={rename}>Rename</button>
        <button class="rounded-md border border-line bg-surface px-5 py-2 text-sm font-medium" onclick={remove}>Delete</button>
      </div>
    {/if}
  {/snippet}

  {#if missing || !pl}
    <EmptyState icon="library" title="This playlist no longer exists" text="It may have been deleted. Your other playlists are in the Library." />
  {:else if pl.entries.length === 0}
    <EmptyState icon="music" title="This playlist is empty" text="Choose Add to playlist on any song to put it here." />
  {:else}
    <ul aria-label="Songs in {pl.name}">
      {#each pl.entries as e, i (e.entryId)}
        <li
          draggable="true"
          class="group flex items-center gap-1 border-b border-t-2 border-b-line/60 hover:bg-surface-2/70 {overIndex === i && dragId !== e.entryId ? 'border-accent' : 'border-transparent'} {dragId === e.entryId ? 'opacity-40' : ''}"
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
          <span class="cursor-grab p-2 text-muted" aria-hidden="true"><Icon name="drag" size={18} /></span>
          <button class="flex min-w-0 flex-1 items-center gap-3 py-2 text-left" onclick={() => play(false, i)}>
            <span class="w-6 shrink-0 text-center text-[12px] tabular-nums text-muted" aria-hidden="true">{i + 1}</span>
            <Thumb src={e.track.thumbnail} seed={e.track.key} size={40} rounded="rounded-sm" />
            <span class="min-w-0 flex-1">
              <span class="block truncate text-[14px] font-medium">{e.track.title}</span>
              <span class="block truncate text-[12px] text-muted">{e.track.artist}</span>
            </span>
            <span class="text-[13px] tabular-nums text-muted">{formatTime(e.track.durationMs)}</span>
          </button>
          <button class="p-1.5 text-muted hover:text-fg disabled:opacity-30" aria-label="Move {e.track.title} up" disabled={i === 0} onclick={() => move(e.entryId, i - 1)}><Icon name="up" size={18} /></button>
          <button class="p-1.5 text-muted hover:text-fg disabled:opacity-30" aria-label="Move {e.track.title} down" disabled={i === pl.entries.length - 1} onclick={() => move(e.entryId, i + 1)}><Icon name="down" size={18} /></button>
          <button class="p-1.5 text-muted hover:text-fg" aria-label="Remove {e.track.title} from playlist" onclick={() => removeEntry(e.entryId, e.track.title)}><Icon name="close" size={18} /></button>
        </li>
      {/each}
    </ul>
  {/if}
</ScrollPage>
