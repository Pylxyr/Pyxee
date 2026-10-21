<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { messageOf } from '$lib/backend';
  import { snackbar } from '$lib/snackbar.svelte';
  import Icon from './Icon.svelte';
  import MenuItem from './MenuItem.svelte';
  import Sheet from './Sheet.svelte';
  import Thumb from './Thumb.svelte';

  const track = $derived(nav.pickTrack);

  async function add(p: { id: number; name: string }) {
    const t = track;
    nav.pickTrack = null;
    if (t) await library.addToPlaylist(p, [t]);
  }

  function create() {
    const t = track;
    nav.pickTrack = null;
    if (!t) return;
    nav.namePrompt = {
      title: 'New playlist',
      initial: '',
      confirm: 'Create',
      run: async (name) => {
        const id = await library.backend!.playlistCreate(name, [t]).catch((e) => Promise.reject(messageOf(e)));
        snackbar.push(`Added to ${name.trim()}`, { action: { label: 'Open', run: () => nav.openPlaylist(id) } });
      },
    };
  }
</script>

<Sheet open={track !== null} title="Add to playlist" onclose={() => (nav.pickTrack = null)}>
  <MenuItem onclick={create}>
    <span class="flex h-10 w-10 items-center justify-center r-sm bg-surface-2"><Icon name="plus" size={18} /></span>
    New playlist…
  </MenuItem>
  <ul class="max-h-72 overflow-y-auto">
    {#each library.playlists as p (p.id)}
      <li>
        <MenuItem onclick={() => add(p)}>
          <Thumb src={p.thumbnail} seed={`pl:${p.id}`} size={40} rounded="r-sm" />
          <span class="min-w-0 flex-1">
            <span class="trow-title block">{p.name}</span>
            <span class="trow-artist block">{p.trackCount} {p.trackCount === 1 ? 'song' : 'songs'}</span>
          </span>
        </MenuItem>
      </li>
    {/each}
  </ul>
</Sheet>
