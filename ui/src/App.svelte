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
  import { updates } from '$lib/updates.svelte';
  import { hasOwnWindow, tauriWindowControls, type WindowControls } from '$lib/windowControls';
  import TitleBar from './components/TitleBar.svelte';
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
  import ChangelogSheet from './components/ChangelogSheet.svelte';
  import Home from './pages/Home.svelte';
  import Explore from './pages/Explore.svelte';
  import Library from './pages/Library.svelte';
  import Search from './pages/Search.svelte';
  import Queue from './pages/Queue.svelte';
  import Playlist from './pages/Playlist.svelte';

  // Wide windows get the tabs on top and the deck along the bottom; narrow ones the bottom bar and the dock.
  const wide = new MediaQuery('min-width: 900px');
  // From 1440 px there is room for the queue lane beside the page.
  const lane = new MediaQuery('min-width: 1440px');

  let backendRef = $state.raw<Backend | null>(null);
  // The main window has no native frame, so the page draws the title bar. Null in a browser, where there is no window to control.
  let controls = $state.raw<WindowControls | null>(null);
  let fullscreen = $state(false);
  // Pyxee's own title bar shows unless the user chose the Windows frame, or the window is full screen (no bar at all then).
  const showTitleBar = $derived(controls != null && shellSettings.settings.borderless && !fullscreen);

  onMount(() => {
    if (hasOwnWindow()) void tauriWindowControls().then((c) => (controls = c)).catch(() => {});
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
      // Look for a newer Pyxee once, now; the changelog opens by itself if there is one.
      void updates.init(backend).catch(() => {}).then(() => updates.checkAtStartup(backend));
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

  // Overlays start below the title bar, not over it, so its buttons stay usable.
  $effect(() => {
    document.documentElement.toggleAttribute('data-titlebar', showTitleBar);
  });

  // Full screen comes and goes with F11 or Windows itself; follow the window rather than assume.
  $effect(() => {
    const c = controls;
    if (!c) return;
    let live = true;
    let unlisten: (() => void) | null = null;
    const sync = () => void c.isFullscreen().then((f) => live && (fullscreen = f)).catch(() => {});
    sync();
    c.onResized(sync).then((off) => (live ? (unlisten = off) : off())).catch(() => {});
    return () => {
      live = false;
      unlisten?.();
    };
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
    const backend = library.backend;
    accentFromImage(url, backend ? (u) => backend.coverBytes(u) : undefined).then((c) => live && (theme.art = c));
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
    if (a.kind === 'fullscreen') {
      // In a browser F11 is the browser's own.
      if (!controls) return;
      e.preventDefault();
      void controls.setFullscreen(!fullscreen).catch(() => {});
      return;
    }
    e.preventDefault();
    if (a.kind === 'close') nav.closeNowPlaying();
    else player.dispatch(a.command);
  }
</script>

<svelte:window {onkeydown} />

<div class="frame">
{#if controls && showTitleBar}<TitleBar {controls} showBrand={!wide.current} />{/if}
<div class="shell" data-layout={wide.current ? 'wide' : 'narrow'} data-lane={wide.current && lane.current ? '' : undefined}>
  {#if wide.current}
    <div class="masthead"><Navigation layout="tabs" /><TopBar showSettings /></div>
  {/if}
  <div class="workspace">
    <main class="page overflow-hidden">
      {#if !wide.current}<TopBar showSettings />{/if}
      {#if library.snapshot.error}
        <p class="notice mb-2 shrink-0" style="margin-inline:var(--page-pad)" role="status">Your library couldn't be opened ({library.snapshot.error}). Likes, playlists and history won't be saved this session.</p>
      {/if}
      <div class="page-host min-h-0 flex-1">
        {#key nav.route}
          <div class="route-in h-full">
            {#if nav.route === 'home'}<Home />
            {:else if nav.route === 'explore'}<Explore />
            {:else if nav.route === 'library'}<Library />
            {:else if nav.route === 'queue'}<Queue />
            {:else if nav.route === 'playlist'}<Playlist />
            {:else}<Search />{/if}
          </div>
        {/key}
      </div>
    </main>
    {#if !wide.current}
      <Dock />
      <Navigation layout="bar" />
    {/if}
  </div>
  {#if wide.current}<Stage lane={lane.current} />{/if}
</div>
</div>

<NowPlaying />
<Snackbar />
<TrackMenu />
<PlaylistPicker />
<NameSheet />
<ThemeEditor />
<SleepSheet />
<SettingsSheet backend={backendRef} />
<ChangelogSheet backend={backendRef} />
