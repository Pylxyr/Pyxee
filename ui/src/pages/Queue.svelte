<script lang="ts">
  import { player } from '$lib/player.svelte';
  import { removeFromQueue, clearQueue } from '$lib/actions';
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { messageOf } from '$lib/backend';
  import { snackbar } from '$lib/snackbar.svelte';
  import { formatTime } from '$lib/format';
  import Icon from '../components/Icon.svelte';
  import Thumb from '../components/Thumb.svelte';
  import EmptyState from '../components/EmptyState.svelte';

  const queue = $derived(player.state.queue);
  const firstRadio = $derived(queue.find((e) => e.origin === 'radio')?.id ?? null);
  let dragId = $state<number | null>(null);
  let overIndex = $state<number | null>(null);

  function saveAsPlaylist() {
    const tracks = queue.map((e) => e.track);
    nav.namePrompt = {
      title: 'Save queue as playlist',
      initial: '',
      confirm: 'Save',
      run: async (name) => {
        const id = await library.backend!.playlistCreate(name, tracks).catch((e) => Promise.reject(messageOf(e)));
        snackbar.push(`Saved ${tracks.length} songs as ${name.trim()}`, { action: { label: 'Open', run: () => nav.openPlaylist(id) } });
      },
    };
  }

  function move(id: number, to: number) {
    if (to < 0 || to >= queue.length) return;
    player.dispatch({ type: 'move', id, to });
  }
  function drop(e: DragEvent, to: number) {
    e.preventDefault();
    if (dragId != null) move(dragId, to);
    dragId = overIndex = null;
  }
  function status(id: number) {
    if (id === player.state.audible) return 'Now playing';
    if (id === player.state.loading) return 'Loading';
    return '';
  }
</script>

<section class="px-4 pb-8 sm:px-8">
  <div class="flex flex-wrap items-center gap-3">
    <h1 class="mr-auto text-2xl font-bold">Queue <span class="text-base font-normal text-muted">{queue.length} {queue.length === 1 ? 'song' : 'songs'}</span></h1>
    <button class="rounded-full bg-surface-2 px-4 py-1.5 text-sm font-medium disabled:opacity-40" disabled={queue.length === 0} onclick={saveAsPlaylist}>Save as playlist</button>
    <button class="rounded-full bg-surface-2 px-4 py-1.5 text-sm font-medium disabled:opacity-40" disabled={queue.length === 0} onclick={clearQueue}>Clear queue</button>
  </div>

  {#if queue.length === 0}
    <EmptyState icon="queue" title="Your queue is empty" text="Search for a song and choose Play next or Add to queue." />
  {:else}
    <ul class="mt-4" aria-label="Queue">
      {#each queue as e, i (e.id)}
        {#if e.id === firstRadio}
          <li class="px-3 pb-1 pt-5 text-xs font-semibold uppercase tracking-wider text-muted" aria-hidden="true">Autoplay · similar songs</li>
        {/if}
        <li
          draggable="true"
          class="group flex items-center gap-1 rounded-lg border-t-2 hover:bg-surface-2 {overIndex === i && dragId !== e.id ? 'border-accent' : 'border-transparent'} {dragId === e.id ? 'opacity-40' : ''} {status(e.id) ? 'bg-surface-2' : ''}"
          ondragstart={(ev) => {
            dragId = e.id;
            ev.dataTransfer?.setData('text/plain', String(e.id));
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
          <button class="flex min-w-0 flex-1 items-center gap-3 py-2 text-left" onclick={() => player.playEntry(e.id)}>
            <Thumb src={e.track.thumbnail} seed={e.track.key} size={48} />
            <span class="min-w-0 flex-1">
              <span class="block truncate text-[15px] font-semibold">{e.track.title}</span>
              <span class="block truncate text-[13px] {status(e.id) ? 'text-accent' : 'text-muted'}">{status(e.id) || e.track.artist}</span>
            </span>
            <span class="text-[13px] tabular-nums text-muted">{formatTime(e.track.durationMs)}</span>
          </button>
          <button class="p-1.5 text-muted hover:text-fg disabled:opacity-30" aria-label="Move {e.track.title} up" disabled={i === 0} onclick={() => move(e.id, i - 1)}><Icon name="up" size={18} /></button>
          <button class="p-1.5 text-muted hover:text-fg disabled:opacity-30" aria-label="Move {e.track.title} down" disabled={i === queue.length - 1} onclick={() => move(e.id, i + 1)}><Icon name="down" size={18} /></button>
          <button class="p-1.5 text-muted hover:text-fg" aria-label="Remove {e.track.title} from queue" onclick={() => removeFromQueue(e)}><Icon name="close" size={18} /></button>
        </li>
      {/each}
    </ul>
  {/if}
</section>
