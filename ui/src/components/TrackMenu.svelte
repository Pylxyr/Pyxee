<script lang="ts">
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { library } from '$lib/library.svelte';
  import { search } from '$lib/search.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import MenuItem from './MenuItem.svelte';
  import Sheet from './Sheet.svelte';

  // The menu a row's "More options" button opens: an in-page sheet, never a native menu.
  function done(label: string, run: () => void) {
    run();
    nav.menuTrack = null;
    snackbar.push(label);
  }
  /** Closes the menu first, then does something that opens another sheet or page. The track is
   *  captured before closing: the menu's `{@const}` goes null the moment `menuTrack` is cleared. */
  function leave(run: (track: TrackDto) => void) {
    const track = nav.menuTrack;
    if (!track) return;
    nav.menuTrack = null;
    run(track);
  }
</script>

<Sheet open={nav.menuTrack !== null} title={nav.menuTrack?.title ?? ''} onclose={() => (nav.menuTrack = null)}>
  {#if nav.menuTrack}
    {@const t = nav.menuTrack}
    <MenuItem onclick={() => done('Playing now', () => player.playTracks([t]))}>Play now</MenuItem>
    <MenuItem onclick={() => done('Will play next', () => player.playNext(t))}>Play next</MenuItem>
    <MenuItem onclick={() => done('Added to queue', () => player.addToQueue(t))}>Add to queue</MenuItem>
    <MenuItem onclick={() => leave((track) => library.toggleLike(track))}>{library.isLiked(t.key) ? 'Unlike' : 'Like'}</MenuItem>
    <MenuItem onclick={() => leave((track) => (nav.pickTrack = track))}>Add to playlist</MenuItem>
    <MenuItem onclick={() => leave((track) => { nav.go('search'); search.run(track.artist, 'songs'); })}>Go to artist</MenuItem>
  {/if}
</Sheet>
