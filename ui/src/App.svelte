<script lang="ts">
  import { onMount } from 'svelte';
  import { MediaQuery } from 'svelte/reactivity';
  import { createBackend, type Backend } from '$lib/backend';
  import { player } from '$lib/player.svelte';
  import { search } from '$lib/search.svelte';
  import { library } from '$lib/library.svelte';
  import { local } from '$lib/local.svelte';
  import { prefs } from '$lib/prefs.svelte';
  import { nav } from '$lib/nav.svelte';
  import { followArt, theme } from '$lib/theme.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { actionForKey } from '$lib/shortcuts';
  import { lyricsStore } from '$lib/lyrics.svelte';
  import { relatedStore } from '$lib/related.svelte';
  import { sleep } from '$lib/sleep.svelte';
  import { eq } from '$lib/eq.svelte';
  import { shellSettings } from '$lib/shell.svelte';
  import { updates } from '$lib/updates.svelte';
  import { hasOwnWindow, tauriWindowControls, type WindowControls } from '$lib/windowControls';
  import { watchUiScale } from '$lib/uiscale';
  import { heroTones } from '$lib/color';
  import { coverColorsOf } from '$lib/covercolors.svelte';
  import Navigation from './components/Navigation.svelte';
  import TopBar from './components/TopBar.svelte';
  import Dock from './components/Dock.svelte';
  import Stage from './components/Stage.svelte';
  import NowPlaying from './components/NowPlaying.svelte';
  import { startPerfIfEnabled } from '$lib/perf';
  import { initSeekMode } from '$lib/seekmode.svelte';
  import { look } from '$lib/look.svelte';
  import { initPlatform } from '$lib/platform.svelte';
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

  // Wide windows get the dock on the left, the search on top and the player along the bottom; narrow ones the bottom bar and the mini player.
  const wide = new MediaQuery('min-width: 900px');
  // From 1440 px there is room for the queue lane beside the page.
  const lane = new MediaQuery('min-width: 1440px');

  // The queue lane takes its column from 1440 px, unless the person hid it. The Queue page shows the queue itself.
  const showLane = $derived(wide.current && lane.current && !nav.laneHidden && nav.route !== 'queue');

  let backendRef = $state.raw<Backend | null>(null);
  // The window's full-screen state (F11). Null in a browser, where there is no window to control.
  let controls = $state.raw<WindowControls | null>(null);
  let fullscreen = $state(false);

  // The interface size follows Settings (and, on automatic, the window's width). Does nothing in a browser.
  $effect(() => {
    const choice = prefs.uiScale;
    return watchUiScale(() => choice);
  });

  onMount(() => {
    if (hasOwnWindow()) void tauriWindowControls().then((c) => (controls = c)).catch(() => {});
    (async () => {
      const backend = await createBackend();
      backendRef = backend;
      await backend.onNotice((message) => snackbar.push(message, { timeoutMs: 10_000 }));
      search.init(backend);
      library.init(backend);
      void local.init(backend).catch(() => {});
      lyricsStore.init(backend);
      relatedStore.init(backend);
      void sleep.init(backend);
      if (__DEV_TOOLS__) void startPerfIfEnabled(backend);
      void initSeekMode(backend);
      void initPlatform(backend);
      void shellSettings.init(backend);
      void eq.init(backend);
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

  // Likewise the crossfade between songs (Settings -> Playback).
  $effect(() => {
    if (player.ready) void player.setCrossfade(prefs.crossfade);
  });

  // The native window (title bar, Mica) follows the chosen theme, not only the Windows setting.
  $effect(() => {
    const wanted = theme.windowTheme;
    if (player.ready) void backendRef?.setWindowTheme(wanted);
  });

  // With "Color from album art" the native window border (and the Windows title bar) wear the cover's colour too.
  $effect(() => {
    const tint = theme.nativeTint;
    if (player.ready) void backendRef?.setWindowTint(tint);
  });

  // The look chosen in Settings > Appearance is data attributes on the root element; the rules are in app.css.
  $effect(() => {
    void look.value;
    look.apply();
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

  // The window's canvas takes one soft colour from the cover of the song that is playing (`--canvas-tint`, read by the
  // shell's background; Home overrides it with its own top card's cover while nothing plays). It changes with the song.
  const canvas = coverColorsOf(() => player.shown?.track.thumbnail ?? null);
  $effect(() => {
    const tone = canvas.value ? heroTones(canvas.value).a : null;
    const root = document.documentElement;
    if (tone) root.style.setProperty('--canvas-tint', tone);
    else root.style.removeProperty('--canvas-tint');
  });

  // Keep <html> in sync with theme, backdrop and accent.
  $effect(() => theme.apply(document.documentElement));

  // "Color from album art": the palette follows the cover of the song that is playing.
  followArt(() => player.shown?.track.thumbnail ?? null, () => library.backend);

  // Whether the focus is where it is because of a mouse click (a clicked song row keeps focus). Space then means
  // play/pause instead of "press the row again"; Tab hands focus back to the keyboard.
  let pointerFocus = false;
  // The keyup of a Space we took: a focused button would otherwise be clicked by it (WebView2 clicks on key release).
  let swallowSpaceUp = false;

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Tab') pointerFocus = false;
    const a = actionForKey(e, {
      positionMs: player.positionMs,
      durationMs: player.durationMs,
      nowPlayingOpen: nav.nowPlayingOpen,
      hasTrack: player.shown != null,
      pointerFocus,
    });
    if (!a) return;
    if (a.kind === 'search') {
      // Not under a sheet or Now Playing: the box is behind them.
      if (document.querySelector('[data-sheet]')) return;
      const input = document.querySelector<HTMLInputElement>('input[data-search-input]');
      if (!input) return;
      e.preventDefault();
      input.focus();
      input.select();
      return;
    }
    if (e.key === ' ' && e.target instanceof HTMLButtonElement) swallowSpaceUp = true;
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

<svelte:window
  {onkeydown}
  onpointerdowncapture={() => (pointerFocus = true)}
  onkeyup={(e) => {
    if (e.key === ' ' && swallowSpaceUp) {
      swallowSpaceUp = false;
      e.preventDefault();
    }
  }}
/>

<div class="frame">
<div class="shell" inert={nav.npCovering} data-covered={nav.npCovering ? '' : undefined} data-layout={wide.current ? 'wide' : 'narrow'} data-lane={showLane ? '' : undefined}>
  {#if wide.current}
    <Navigation layout="rail" />
    <div class="masthead"><TopBar /></div>
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
  {#if wide.current}<Stage lane={showLane} room={lane.current} />{/if}
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
