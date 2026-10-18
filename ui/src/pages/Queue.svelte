<script lang="ts">
  import { player } from '$lib/player.svelte';
  import { removeFromQueue, clearQueue } from '$lib/actions';
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { messageOf } from '$lib/backend';
  import { snackbar } from '$lib/snackbar.svelte';
  import { formatTime } from '$lib/format';
  import { reorder } from '$lib/reorder';
  import Icon from '../components/Icon.svelte';
  import Thumb from '../components/Thumb.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';

  const queue = $derived(player.state.queue);
  const firstRadio = $derived(queue.find((e) => e.origin === 'radio')?.id ?? null);
  /** The row being dragged and where it would land (null when nothing is being dragged). */
  let drag = $state<{ id: number; from: number; to: number } | null>(null);
  let listEl = $state<HTMLElement>();
  const rows = () => [...(listEl?.querySelectorAll<HTMLElement>('[data-row]') ?? [])];

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
  function status(id: number) {
    if (id === player.state.audible) return 'Now playing';
    if (id === player.state.loading) return 'Loading';
    return '';
  }
</script>

<ScrollPage>
  {#snippet header()}
  <div class="flex flex-wrap items-center gap-3">
    <h1 class="mr-auto text-2xl font-bold">Queue <span class="text-base font-normal text-muted">{queue.length} {queue.length === 1 ? 'song' : 'songs'}</span></h1>
    <button class="rounded-md border border-line bg-surface px-4 py-1.5 text-sm font-medium disabled:opacity-40" disabled={queue.length === 0} onclick={saveAsPlaylist}>Save as playlist</button>
    <button class="rounded-md border border-line bg-surface px-4 py-1.5 text-sm font-medium disabled:opacity-40" disabled={queue.length === 0} onclick={clearQueue}>Clear queue</button>
  </div>
  {/snippet}

  {#if queue.length === 0}
    <EmptyState icon="queue" title="Your queue is empty" text="Search for a song and choose Play next or Add to queue." />
  {:else}
    <ul class="mt-4" aria-label="Queue" bind:this={listEl}>
      {#each queue as e, i (e.id)}
        {#if e.id === firstRadio}
          <li class="px-3 pb-1 pt-5 text-xs font-semibold uppercase tracking-wider text-muted" aria-hidden="true">Autoplay · similar songs</li>
        {/if}
        <li
          data-row
          class="group flex items-center gap-1 rounded-lg border-y-2 border-transparent hover:bg-surface-2 {drag && drag.to === i && drag.id !== e.id ? (drag.to < drag.from ? 'border-t-accent' : 'border-b-accent') : ''} {drag?.id === e.id ? 'opacity-40' : ''} {status(e.id) ? 'bg-surface-2' : ''}"
        >
          <span
            class="cursor-grab touch-none select-none p-2 text-muted"
            role="presentation"
            title="Drag to reorder"
            data-testid="drag-handle"
            use:reorder={{
              index: i,
              rows,
              onover: (to) => (drag = to === null ? null : { id: e.id, from: i, to }),
              ondrop: (to) => move(e.id, to),
            }}><Icon name="drag" size={18} /></span>
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
</ScrollPage>
