<script lang="ts">
  import { local } from '$lib/local.svelte';
  import { library } from '$lib/library.svelte';
  import { player } from '$lib/player.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { messageOf } from '$lib/backend';
  import { Loaded } from '$lib/loaded.svelte';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import Button from './Button.svelte';
  import EmptyState from './EmptyState.svelte';
  import Icon from './Icon.svelte';
  import Skeleton from './Skeleton.svelte';
  import TrackList from './TrackList.svelte';

  let query = $state('');
  const source = new Loaded<TrackDto[]>();
  const tracks = $derived(source.data ?? []);
  const info = $derived(local.info);

  // The list is read again whenever the folders or the scan result change, and as the filter is typed.
  $effect(() => {
    void info.revision;
    const q = query;
    const b = library.backend;
    if (!b) return;
    void source.load(() => b.localTracks(q));
  });

  const songs = (n: number) => `${n} ${n === 1 ? 'song' : 'songs'}`;
  const status = $derived(info.scanning ? `Reading your folders… ${songs(info.trackCount)} so far` : songs(info.trackCount));

  async function addFolder() {
    try {
      await library.backend!.localAddFolder();
    } catch (e) {
      snackbar.push(messageOf(e));
    }
  }

  async function removeFolder(folder: string) {
    try {
      await library.backend!.localRemoveFolder(folder);
      snackbar.push('Folder removed from Pyxee. Your files are untouched.');
    } catch (e) {
      snackbar.push(messageOf(e));
    }
  }

  async function rescan() {
    try {
      await library.backend!.localRescan();
    } catch (e) {
      snackbar.push(messageOf(e));
    }
  }

  function playAll(shuffle: boolean) {
    if (tracks.length === 0) return;
    if (shuffle !== player.state.shuffle) player.toggleShuffle();
    player.playTracks(tracks, 0);
  }
</script>

{#if info.folders.length === 0}
  <EmptyState
    icon="music"
    title="Play the music on this PC"
    text="Choose the folders your music is in. Pyxee reads the titles and artists from your MP3, FLAC, Ogg, Opus, M4A, AAC and WAV files and keeps the list up to date."
  />
  <div class="flex justify-center">
    <Button variant="primary" onclick={addFolder}><Icon name="plus" size={18} />Add a folder</Button>
  </div>
{:else}
  <div class="toolbar">
    <Button variant="primary" onclick={addFolder}><Icon name="plus" size={18} />Add a folder</Button>
    <Button onclick={rescan} disabled={info.scanning}>Rescan</Button>
    <span class="muted ml-auto" role="status">{status}</span>
  </div>
  <ul class="my-3 flex flex-col gap-1" aria-label="Music folders">
    {#each info.folders as folder (folder)}
      <li class="flex items-center gap-2">
        <span class="muted min-w-0 flex-1 truncate" title={folder}>{folder}</span>
        <Button size="sm" variant="quiet" aria-label="Remove {folder}" onclick={() => removeFolder(folder)}><Icon name="close" size={16} /></Button>
      </li>
    {/each}
  </ul>
  <input class="field mb-3" type="search" aria-label="Search your songs" placeholder="Search your songs" autocomplete="off" bind:value={query} />
  {#if source.error}<p class="accent-text mb-3" role="alert">{source.error}</p>{/if}
  {#if !source.loaded && !source.error}
    <Skeleton rows={8} />
  {:else if tracks.length === 0}
    <EmptyState
      icon="music"
      title={query.trim() ? 'No matches' : info.scanning ? 'Reading your folders' : 'No songs found'}
      text={query.trim() ? 'Nothing on this PC matches that.' : info.scanning ? 'Songs show up here as soon as they are read.' : 'These folders hold no MP3, FLAC, Ogg, Opus, M4A, AAC or WAV files yet.'}
    />
  {:else}
    <div class="toolbar">
      <Button variant="primary" onclick={() => playAll(false)}>Play all</Button>
      <Button onclick={() => playAll(true)}>Shuffle</Button>
    </div>
    <TrackList label="Songs on this PC" {tracks} onplay={(i) => player.playTracks(tracks, i)} />
  {/if}
{/if}
