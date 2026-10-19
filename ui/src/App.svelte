<script lang="ts">
  import UpdatesSection from './components/UpdatesSection.svelte';
  import { onMount } from 'svelte';
  import { MediaQuery } from 'svelte/reactivity';
  import { createBackend, messageOf, type Backend } from '$lib/backend';
  import type { LogInfoDto } from '$lib/bindings/LogInfoDto';
  import { player } from '$lib/player.svelte';
  import { search } from '$lib/search.svelte';
  import { library } from '$lib/library.svelte';
  import { prefs } from '$lib/prefs.svelte';
  import { nav } from '$lib/nav.svelte';
  import { theme } from '$lib/theme.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { accentFromImage } from '$lib/color';
  import { actionForKey } from '$lib/shortcuts';
  import DeckNav from './components/DeckNav.svelte';
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
  import ThemePicker from './components/ThemePicker.svelte';
  import ThemeEditor from './components/ThemeEditor.svelte';
  import PlaylistPicker from './components/PlaylistPicker.svelte';
  import SleepSheet from './components/SleepSheet.svelte';
  import { lyricsStore } from '$lib/lyrics.svelte';
  import { relatedStore } from '$lib/related.svelte';
  import { sleep } from '$lib/sleep.svelte';
  import { shellSettings } from '$lib/shell.svelte';
  import { sleepLabel } from '$lib/lyrics';

  const wide = new MediaQuery('min-width: 900px');

  let backendRef = $state.raw<Backend | null>(null);
  let logInfo = $state<LogInfoDto | null>(null);

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

  // The log location is looked up when the Settings sheet (which has the Diagnostics section) opens.
  $effect(() => {
    if (nav.themeSheetOpen && backendRef) void backendRef.logInfo().then((i) => (logInfo = i));
  });

  async function copyLogPath() {
    try {
      await navigator.clipboard.writeText(logInfo?.file ?? '');
      snackbar.push('Log file path copied');
    } catch {
      snackbar.push('Could not copy. Select the path and copy it by hand.');
    }
  }

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

  function menuDo(label: string, run: () => void) {
    run();
    nav.menuTrack = null;
    snackbar.push(label);
  }
</script>

<svelte:window {onkeydown} />

<div class="flex h-dvh flex-col bg-bg text-fg">
  <div class="flex min-h-0 flex-1">
    <main class="flex min-w-0 flex-1 flex-col overflow-hidden">
      <div class="relative z-20 flex shrink-0 items-center gap-4 border-b border-line px-4 sm:px-6">
        {#if wide.current}<DeckNav />{/if}
        <div class="min-w-0 flex-1"><TopBar /></div>
        <button class="rounded-md p-2 text-muted hover:text-fg" aria-label="Settings" onclick={() => (nav.themeSheetOpen = true)}>
          <Icon name="settings" size={20} />
        </button>
      </div>
      {#if library.snapshot.error}
        <p class="mx-4 mb-2 shrink-0 rounded-xl bg-surface-2 px-4 py-3 text-sm sm:mx-8" role="status">Your library couldn't be opened ({library.snapshot.error}). Likes, playlists and history won't be saved this session.</p>
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
<ThemeEditor />
<SleepSheet />

<Sheet open={nav.themeSheetOpen} title="Settings" side="right" onclose={() => (nav.themeSheetOpen = false)}>
  <section aria-label="Appearance">
    <h3 class="px-3 pb-1 text-sm font-semibold">Appearance</h3>
    <ThemePicker />
    <label class="flex items-center justify-between rounded-lg px-3 py-3 hover:bg-surface">
      <span>Color from album art</span>
      <input type="checkbox" checked={theme.settings.dynamicColor} onchange={(e) => theme.update({ dynamicColor: e.currentTarget.checked })} />
    </label>
  </section>
  <section class="mt-3 border-t border-line pt-3" aria-label="Windows">
    <h3 class="px-3 pb-1 text-sm font-semibold">Windows</h3>
    <label class="flex items-center justify-between rounded-lg px-3 py-3 hover:bg-surface">
      <span>Keep playing in the tray when the window is closed</span>
      <input type="checkbox" checked={shellSettings.settings.closeToTray} onchange={(e) => shellSettings.update({ closeToTray: e.currentTarget.checked })} />
    </label>
    <label class="flex items-center justify-between rounded-lg px-3 py-3 hover:bg-surface">
      <span>Start Pyxee when I sign in to Windows</span>
      <input type="checkbox" checked={shellSettings.settings.startWithWindows} onchange={(e) => shellSettings.update({ startWithWindows: e.currentTarget.checked })} />
    </label>
    <label class="flex items-center justify-between rounded-lg px-3 py-3 hover:bg-surface">
      <span>Show a notification when the song changes</span>
      <input type="checkbox" checked={shellSettings.settings.toastOnChange} onchange={(e) => shellSettings.update({ toastOnChange: e.currentTarget.checked })} />
    </label>
    <label class="flex items-center justify-between gap-3 rounded-lg px-3 py-3 hover:bg-surface">
      <span>Show previous, play and next buttons in the notification area</span>
      <input type="checkbox" checked={shellSettings.settings.trayControls} onchange={(e) => shellSettings.update({ trayControls: e.currentTarget.checked })} />
    </label>
    <p class="px-3 pb-1 text-[13px] text-muted">
      They sit beside the Pyxee icon, and the song title shows when you point at it. Windows may put new icons in the
      hidden-icons flyout (the ^ arrow): drag them out onto the taskbar once and Windows remembers.
    </p>
    <label class="flex items-center justify-between gap-3 rounded-lg px-3 py-3 hover:bg-surface">
      <span>Clicking the Pyxee tray icon opens the mini controls (otherwise it opens the window)</span>
      <input type="checkbox" checked={shellSettings.settings.flyoutOnClick} onchange={(e) => shellSettings.update({ flyoutOnClick: e.currentTarget.checked })} />
    </label>
    <label class="flex items-center justify-between gap-3 rounded-lg px-3 py-3 hover:bg-surface">
      <span>Show what I'm playing on Discord</span>
      <input type="checkbox" checked={shellSettings.settings.discordPresence} onchange={(e) => shellSettings.update({ discordPresence: e.currentTarget.checked })} />
    </label>
    <p class="px-3 pb-1 text-[13px] text-muted">
      Shows "Listening to Pyxee" with the current song in the Discord desktop app, while it is running.
    </p>
    {#if shellSettings.error}<p class="px-3 text-[13px] text-accent" role="alert">Could not save that setting ({shellSettings.error}).</p>{/if}
    <button class="flex w-full items-center justify-between rounded-lg px-3 py-3 text-left hover:bg-surface" onclick={() => { nav.themeSheetOpen = false; nav.sleepSheetOpen = true; }}>
      <span>Sleep timer</span><span class="text-sm text-muted">{sleepLabel(sleep.status)}</span>
    </button>
  </section>
  {#if backendRef}<UpdatesSection backend={backendRef} />{/if}

  <section class="mt-3 border-t border-line px-3 pt-3" aria-label="Diagnostics">
    <h3 class="text-sm font-semibold">Diagnostics</h3>
    <p class="mt-1 text-[13px] text-muted">Pyxee keeps a log of what it does. If a song won't play or keeps skipping, the log says why.</p>
    {#if logInfo}
      <p class="mt-2 select-all break-all rounded-lg bg-surface px-3 py-2 text-[12px]" data-testid="log-path">{logInfo.file}</p>
      {#if logInfo.error}<p class="mt-1 text-[13px] text-accent" role="alert">{logInfo.error}</p>{/if}
      <div class="mt-2 flex gap-2">
        <button class="rounded-md bg-fg px-4 py-1.5 text-sm font-semibold text-on-fg" onclick={() => backendRef?.openLogFolder().catch((e) => snackbar.push(messageOf(e)))}>Open log folder</button>
        <button class="rounded-md border border-line bg-surface px-4 py-1.5 text-sm font-medium" onclick={copyLogPath}>Copy path</button>
      </div>
    {/if}
  </section>
</Sheet>
