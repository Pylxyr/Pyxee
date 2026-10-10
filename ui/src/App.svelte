<script lang="ts">
  import { onMount } from 'svelte';
  import { MediaQuery } from 'svelte/reactivity';
  import { createBackend } from '$lib/backend';
  import { player } from '$lib/player.svelte';
  import { search } from '$lib/search.svelte';
  import { library } from '$lib/library.svelte';
  import { nav } from '$lib/nav.svelte';
  import { theme, type ThemeMode } from '$lib/theme.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { accentFromImage } from '$lib/color';
  import { actionForKey } from '$lib/shortcuts';
  import SideRail from './components/SideRail.svelte';
  import BottomNav from './components/BottomNav.svelte';
  import TopBar from './components/TopBar.svelte';
  import PlayerBar from './components/PlayerBar.svelte';
  import NowPlaying from './components/NowPlaying.svelte';
  import Snackbar from './components/Snackbar.svelte';
  import Sheet from './components/Sheet.svelte';
  import Icon from './components/Icon.svelte';
  import Home from './pages/Home.svelte';
  import Explore from './pages/Explore.svelte';
  import Library from './pages/Library.svelte';
  import Search from './pages/Search.svelte';
  import Queue from './pages/Queue.svelte';
  import Playlist from './pages/Playlist.svelte';
  import NameSheet from './components/NameSheet.svelte';
  import PlaylistPicker from './components/PlaylistPicker.svelte';

  const wide = new MediaQuery('min-width: 900px');

  onMount(() => {
    (async () => {
      const backend = await createBackend();
      search.init(backend);
      library.init(backend);
      await player.init(backend);
      theme.backdrop = (await backend.windowInfo()).backdrop;
    })();
  });

  // Keep <html> in sync with theme, backdrop and accent.
  $effect(() => theme.apply(document.documentElement));

  // Optional dynamic accent from the current artwork.
  $effect(() => {
    const url = player.shown?.track.thumbnail ?? null;
    if (!theme.settings.dynamicColor || !url) {
      theme.art = null;
      return;
    }
    let live = true;
    accentFromImage(url).then((c) => live && (theme.art = c));
    return () => (live = false);
  });

  function onkeydown(e: KeyboardEvent) {
    const a = actionForKey(e, {
      positionMs: player.state.positionMs,
      durationMs: player.durationMs,
      nowPlayingOpen: nav.nowPlayingOpen,
      hasTrack: player.shown != null,
    });
    if (!a) return;
    e.preventDefault();
    if (a.kind === 'close') nav.closeNowPlaying();
    else player.dispatch(a.command);
  }

  function menuDo(label: string, run: () => void) {
    run();
    nav.menuTrack = null;
    snackbar.push(label);
  }

  const modes: { id: ThemeMode; label: string }[] = [
    { id: 'system', label: 'System' },
    { id: 'dark', label: 'Dark' },
    { id: 'light', label: 'Light' },
  ];
</script>

<svelte:window {onkeydown} />

<div class="flex h-dvh flex-col bg-bg text-fg">
  <div class="flex min-h-0 flex-1">
    {#if wide.current}<SideRail />{/if}
    <main class="min-w-0 flex-1 overflow-y-auto">
      <div class="sticky top-0 z-10 flex items-center">
        <div class="flex-1"><TopBar /></div>
        <button class="mr-4 rounded-full p-2 text-muted hover:text-fg sm:mr-8" aria-label="Appearance" onclick={() => (nav.themeSheetOpen = true)}>
          <Icon name="explore" size={20} />
        </button>
      </div>
      {#if library.snapshot.error}
        <p class="mx-4 mb-2 rounded-xl bg-surface-2 px-4 py-3 text-sm sm:mx-8" role="status">Your library couldn't be opened ({library.snapshot.error}). Likes, playlists and history won't be saved this session.</p>
      {/if}
      {#if nav.route === 'home'}<Home />
      {:else if nav.route === 'explore'}<Explore />
      {:else if nav.route === 'library'}<Library />
      {:else if nav.route === 'queue'}<Queue />
      {:else if nav.route === 'playlist'}<Playlist />
      {:else}<Search />{/if}
    </main>
  </div>
  <PlayerBar wide={wide.current} />
  {#if !wide.current}<BottomNav />{/if}
</div>

<NowPlaying />
<Snackbar />

<Sheet open={nav.menuTrack !== null} title={nav.menuTrack?.title ?? ''} onclose={() => (nav.menuTrack = null)}>
  {#if nav.menuTrack}
    {@const t = nav.menuTrack}
    <button class="flex w-full rounded-lg px-3 py-3 text-left hover:bg-surface" onclick={() => menuDo('Playing now', () => player.playTracks([t]))}>Play now</button>
    <button class="flex w-full rounded-lg px-3 py-3 text-left hover:bg-surface" onclick={() => menuDo('Will play next', () => player.playNext(t))}>Play next</button>
    <button class="flex w-full rounded-lg px-3 py-3 text-left hover:bg-surface" onclick={() => menuDo('Added to queue', () => player.addToQueue(t))}>Add to queue</button>
    <button class="flex w-full rounded-lg px-3 py-3 text-left hover:bg-surface" onclick={() => { const track = t; nav.menuTrack = null; library.toggleLike(track); }}>{library.isLiked(t.key) ? 'Unlike' : 'Like'}</button>
    <button class="flex w-full rounded-lg px-3 py-3 text-left hover:bg-surface" onclick={() => { const track = t; nav.menuTrack = null; nav.pickTrack = track; }}>Add to playlist</button>
    <button class="flex w-full rounded-lg px-3 py-3 text-left hover:bg-surface" onclick={() => { const artist = t.artist; nav.menuTrack = null; nav.go('search'); search.run(artist, 'songs'); }}>Go to artist</button>
  {/if}
</Sheet>

<PlaylistPicker />
<NameSheet />

<Sheet open={nav.themeSheetOpen} title="Appearance" onclose={() => (nav.themeSheetOpen = false)}>
  <div class="flex gap-2 px-2 pb-3" role="group" aria-label="Theme">
    {#each modes as m (m.id)}
      <button
        class="flex-1 rounded-full px-4 py-2 text-sm font-medium {theme.settings.mode === m.id ? 'bg-fg text-bg' : 'bg-surface text-fg'}"
        aria-pressed={theme.settings.mode === m.id}
        onclick={() => theme.update({ mode: m.id })}>{m.label}</button>
    {/each}
  </div>
  <label class="flex items-center justify-between rounded-lg px-3 py-3 hover:bg-surface">
    <span>Color from album art</span>
    <input type="checkbox" checked={theme.settings.dynamicColor} onchange={(e) => theme.update({ dynamicColor: e.currentTarget.checked })} />
  </label>
</Sheet>
