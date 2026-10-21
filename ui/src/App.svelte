<script lang="ts">
  import { onMount } from 'svelte';
  import { MediaQuery } from 'svelte/reactivity';
  import { createBackend, type Backend } from '$lib/backend';
  import { player } from '$lib/player.svelte';
  import { search } from '$lib/search.svelte';
  import { library } from '$lib/library.svelte';
  import { prefs } from '$lib/prefs.svelte';
  import { nav } from '$lib/nav.svelte';
  import { theme } from '$lib/theme.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { accentFromImage } from '$lib/color';
  import { actionForKey } from '$lib/shortcuts';
  import { lyricsStore } from '$lib/lyrics.svelte';
  import { relatedStore } from '$lib/related.svelte';
  import { sleep } from '$lib/sleep.svelte';
  import { shellSettings } from '$lib/shell.svelte';
  import Navigation from './components/Navigation.svelte';
  import TopBar from './components/TopBar.svelte';
  import Dock from './components/Dock.svelte';
  import Stage from './components/Stage.svelte';
  import NowPlaying from './components/NowPlaying.svelte';
  import Snackbar from './components/Snackbar.svelte';
  import TrackMenu from './components/TrackMenu.svelte';
  import SettingsSheet from './components/SettingsSheet.svelte';
  import NameSheet from './components/NameSheet.svelte';
  import ThemeEditor from './components/ThemeEditor.svelte';
  import PlaylistPicker from './components/PlaylistPicker.svelte';
  import SleepSheet from './components/SleepSheet.svelte';
  import Home from './pages/Home.svelte';
  import Explore from './pages/Explore.svelte';
  import Library from './pages/Library.svelte';
  import Search from './pages/Search.svelte';
  import Queue from './pages/Queue.svelte';
  import Playlist from './pages/Playlist.svelte';

  // Wide windows get the side rail and the stage; narrow ones the bottom bar and the dock.
  const wide = new MediaQuery('min-width: 900px');

  let backendRef = $state.raw<Backend | null>(null);

  onMount(() => {
    (async () => {
      const backend = await createBackend();
      backendRef = backend;
      await backend.onNotice((message) => snackbar.push(message, { timeoutMs: 10_000 }));
      search.init(backend);
      library.init(backend);
      lyricsStore.init(backend);
      relatedStore.init(backend);
      void sleep.init(backend);
      void shellSettings.init(backend);
      await player.init(backend);
      theme.backdrop = (await backend.windowInfo()).backdrop;
    })();
  });

  // The volume lives in the UI preferences; the audio engine gets it at start-up and on every change.
  $effect(() => {
    if (player.ready) void player.setVolume(prefs.volume);
  });

  // The native window (title bar, Mica) follows the chosen theme, not only the Windows setting.
  $effect(() => {
    const wanted = theme.windowTheme;
    if (player.ready) void backendRef?.setWindowTheme(wanted);
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
</script>

<svelte:window {onkeydown} />

<div class="shell" data-layout={wide.current ? 'wide' : 'narrow'}>
  {#if wide.current}<Navigation layout="rail" />{/if}
  <div class="workspace">
    <main class="page overflow-hidden">
      <TopBar showSettings={!wide.current} />
      {#if library.snapshot.error}
        <p class="notice mb-2 shrink-0" style="margin-inline:var(--page-pad)" role="status">Your library couldn't be opened ({library.snapshot.error}). Likes, playlists and history won't be saved this session.</p>
      {/if}
      <div class="min-h-0 flex-1">
        {#if nav.route === 'home'}<Home />
        {:else if nav.route === 'explore'}<Explore />
        {:else if nav.route === 'library'}<Library />
        {:else if nav.route === 'queue'}<Queue />
        {:else if nav.route === 'playlist'}<Playlist />
        {:else}<Search />{/if}
      </div>
    </main>
    {#if !wide.current}
      <Dock />
      <Navigation layout="bar" />
    {/if}
  </div>
  {#if wide.current}<Stage />{/if}
</div>

<NowPlaying />
<Snackbar />
<TrackMenu />
<PlaylistPicker />
<NameSheet />
<ThemeEditor />
<SleepSheet />
<SettingsSheet backend={backendRef} />
