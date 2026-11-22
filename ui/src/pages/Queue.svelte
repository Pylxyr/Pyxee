<script lang="ts">
  import { player } from '$lib/player.svelte';
  import { removeFromQueue, clearQueue, queueMenu } from '$lib/actions';
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { messageOf } from '$lib/backend';
  import { snackbar } from '$lib/snackbar.svelte';
  import { reorder } from '$lib/reorder';
  import { focusNeighbour } from '$lib/focus';
  import { flipList } from '$lib/flip';
  import { artistOf, formatSpan, formatTime, titleOf } from '$lib/format';
  import { markListChange } from '$lib/motion';
  import Button from '../components/Button.svelte';
  import EditableRow from '../components/EditableRow.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import IconButton from '../components/IconButton.svelte';
  import LikeButton from '../components/LikeButton.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import PlayButton from '../components/PlayButton.svelte';
  import ScrollPage from '../components/ScrollPage.svelte';
  import Thumb from '../components/Thumb.svelte';

  const queue = $derived(player.state.queue);
  const firstRadio = $derived(queue.find((e) => e.origin === 'radio')?.id ?? null);
  // What is left to hear: the songs after the one that plays now.
  const upNext = $derived(player.upNext);
  const left = $derived(formatSpan(upNext.reduce((sum, e) => sum + (e.track.durationMs ?? 0), 0)));
  const total = $derived(formatSpan(queue.reduce((sum, e) => sum + (e.track.durationMs ?? 0), 0)));
  const now = $derived(player.shown);
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
  const overOf = (id: number, i: number): 'above' | 'below' | null =>
    drag && drag.to === i && drag.id !== id ? (drag.to < drag.from ? 'above' : 'below') : null;
</script>

<ScrollPage>
  {#snippet header()}
    <PageHeader title="Queue" meta="{queue.length} {queue.length === 1 ? 'song' : 'songs'}" detail={total || null}>
      {#snippet actions()}
        <Button disabled={queue.length === 0} onclick={saveAsPlaylist}>Save as playlist</Button>
        <Button disabled={queue.length === 0} onclick={clearQueue}>Clear queue</Button>
      {/snippet}
    </PageHeader>
  {/snippet}

  {#if queue.length === 0}
    <EmptyState icon="queue" title="Your queue is empty" text="Search for a song and choose Play next or Add to queue." />
  {:else}
    <div class="queue-split">
      <ul aria-label="Queue" bind:this={listEl} use:flipList>
        {#each queue as e, i (e.id)}
          {#if e.id === firstRadio}
            <li class="t-caption muted px-3 pb-1 pt-5" aria-hidden="true">Autoplay · similar songs</li>
          {/if}
          <EditableRow
            data-row
            track={e.track}
            index={i}
            status={status(e.id)}
            current={status(e.id) !== ''}
            over={overOf(e.id, i)}
            dragging={drag?.id === e.id}
            inQueue
            menuExtras={() => queueMenu(e)}
            onplay={() => player.playEntry(e.id)}
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
                  onover: (to) => (drag = to === null ? null : { id: e.id, from: i, to }),
                  ondrop: (to) => move(e.id, to),
                }}><Icon name="drag" size={18} /></span>
            {/snippet}
            {#snippet actions()}
              <span class="row-tools">
                <IconButton icon="up" iconSize={18} label="Move {e.track.title} up" disabled={i === 0} onclick={() => move(e.id, i - 1)} />
                <IconButton icon="down" iconSize={18} label="Move {e.track.title} down" disabled={i === queue.length - 1} onclick={() => move(e.id, i + 1)} />
              </span>
              <IconButton icon="close" iconSize={18} label="Remove {e.track.title} from queue" onclick={(ev) => { markListChange(); focusNeighbour(ev.currentTarget); removeFromQueue(e); }} />
            {/snippet}
          </EditableRow>
        {/each}
      </ul>

      {#if now}
        <aside class="queue-now" aria-label="Now playing">
          <div class="queue-now-art">
            <Thumb src={now.track.thumbnail} seed={now.track.key} size={320} fill rounded="r-lg" />
          </div>
          <div class="queue-now-text">
            <p class="t-caption queue-now-kicker">{player.isPlaying ? 'Now playing' : 'Paused'}</p>
            <p class="t-lead truncate" dir="auto">{titleOf(now.track)}</p>
            <p class="muted truncate" dir="auto">{artistOf(now.track)}</p>
          </div>
          <div class="queue-now-controls">
            <IconButton icon="prev" label="Previous" size="lg" strong iconSize={22} onclick={player.previous} />
            <PlayButton />
            <IconButton icon="next" label="Next" size="lg" strong iconSize={22} onclick={player.next} />
            <LikeButton track={now.track} />
          </div>
          <p class="t-caption muted nums queue-now-time">{formatTime(player.positionMs)} / {player.durationMs ? formatTime(player.durationMs) : '--:--'}</p>
          <dl class="queue-stats">
            <div><dt>Up next</dt><dd>{upNext.length} {upNext.length === 1 ? 'song' : 'songs'}</dd></div>
            {#if left}<div><dt>Left to play</dt><dd>{left}</dd></div>{/if}
          </dl>
          <Button variant="quiet" size="sm" onclick={() => nav.openNowPlaying()}>Open Now Playing</Button>
        </aside>
      {/if}
    </div>
  {/if}
</ScrollPage>
