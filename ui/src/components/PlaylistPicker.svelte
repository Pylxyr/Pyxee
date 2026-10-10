<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { messageOf } from '$lib/backend';
  import { snackbar } from '$lib/snackbar.svelte';
  import Sheet from './Sheet.svelte';
  import Thumb from './Thumb.svelte';
  import Icon from './Icon.svelte';

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
  <button class="flex w-full items-center gap-3 rounded-lg px-3 py-3 text-left hover:bg-surface" onclick={create}>
    <span class="flex h-10 w-10 items-center justify-center rounded-md bg-surface"><Icon name="plus" size={18} /></span>
    New playlist…
  </button>
  <ul class="max-h-72 overflow-y-auto">
    {#each library.playlists as p (p.id)}
      <li>
        <button class="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left hover:bg-surface" onclick={() => add(p)}>
          <Thumb src={p.thumbnail} seed={`pl:${p.id}`} size={40} />
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[15px] font-semibold">{p.name}</span>
            <span class="block text-[13px] text-muted">{p.trackCount} {p.trackCount === 1 ? 'song' : 'songs'}</span>
          </span>
        </button>
      </li>
    {/each}
  </ul>
</Sheet>
