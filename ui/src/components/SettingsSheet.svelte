<script lang="ts">
  import { MediaQuery } from 'svelte/reactivity';
  import { messageOf, type Backend } from '$lib/backend';
  import type { LogInfoDto } from '$lib/bindings/LogInfoDto';
  import { nav } from '$lib/nav.svelte';
  import { theme } from '$lib/theme.svelte';
  import { sleep } from '$lib/sleep.svelte';
  import { sleepLabel } from '$lib/lyrics';
  import { shellSettings } from '$lib/shell.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import AdvancedSection from './AdvancedSection.svelte';
  import Button from './Button.svelte';
  import Sheet from './Sheet.svelte';
  import ThemePicker from './ThemePicker.svelte';
  import Toggle from './Toggle.svelte';
  import UpdatesSection from './UpdatesSection.svelte';

  let { backend }: { backend: Backend | null } = $props();
  // Same breakpoint as the app shell: on a wide window Settings is the button at the foot of the left rail, so its
  // tab opens from the left; on a narrow one the button is in the top bar's right corner, so the tab opens from there.
  const wide = new MediaQuery('min-width: 900px');
  let logInfo = $state<LogInfoDto | null>(null);

  // The log location is looked up when the sheet (which has the Diagnostics section) opens.
  $effect(() => {
    if (nav.themeSheetOpen && backend) void backend.logInfo().then((i) => (logInfo = i));
  });

  async function copyLogPath() {
    try {
      await navigator.clipboard.writeText(logInfo?.file ?? '');
      snackbar.push('Log file path copied');
    } catch {
      snackbar.push('Could not copy. Select the path and copy it by hand.');
    }
  }
</script>

<Sheet open={nav.themeSheetOpen} title="Settings" side={wide.current ? 'left' : 'right'} onclose={() => (nav.themeSheetOpen = false)}>
  <section class="settings-section" aria-label="Appearance">
    <h3 class="group-title">Appearance</h3>
    <ThemePicker />
    <Toggle label="Color from album art" checked={theme.settings.dynamicColor} onchange={(on) => theme.update({ dynamicColor: on })} />
  </section>

  <section class="settings-section" aria-label="Windows">
    <h3 class="group-title">Windows</h3>
    <Toggle label="Keep playing in the tray when the window is closed" checked={shellSettings.settings.closeToTray} onchange={(on) => shellSettings.update({ closeToTray: on })} />
    <Toggle label="Start Pyxee when I sign in to Windows" checked={shellSettings.settings.startWithWindows} onchange={(on) => shellSettings.update({ startWithWindows: on })} />
    <Toggle label="Show a notification when the song changes" checked={shellSettings.settings.toastOnChange} onchange={(on) => shellSettings.update({ toastOnChange: on })} />
    <Toggle
      label="Show previous, play and next buttons in the notification area"
      hint="They sit beside the Pyxee icon, and the song title shows when you point at it. Windows may put new icons in the hidden-icons flyout (the ^ arrow): drag them out onto the taskbar once and Windows remembers."
      checked={shellSettings.settings.trayControls}
      onchange={(on) => shellSettings.update({ trayControls: on })}
    />
    <Toggle label="Clicking the Pyxee tray icon opens the mini controls (otherwise it opens the window)" checked={shellSettings.settings.flyoutOnClick} onchange={(on) => shellSettings.update({ flyoutOnClick: on })} />
    <Toggle
      label="Show what I'm playing on Discord"
      hint={'Shows "Listening to Pyxee" with the current song in the Discord desktop app, while it is running.'}
      checked={shellSettings.settings.discordPresence}
      onchange={(on) => shellSettings.update({ discordPresence: on })}
    />
    <Toggle
      label="Scrobble what I play to Last.fm"
      hint="Sends the songs you listen to, and the one playing now, to your Last.fm account. The first time, your browser opens so you can allow Pyxee."
      checked={shellSettings.settings.lastfmScrobbling}
      onchange={(on) => shellSettings.update({ lastfmScrobbling: on })}
    />
    <Toggle
      label="Borderless window"
      hint="Pyxee draws its own slim title bar instead of the Windows one. Press F11 for full screen."
      checked={shellSettings.settings.borderless}
      onchange={(on) => shellSettings.update({ borderless: on })}
    />
    {#if shellSettings.error}<p class="accent-text px-3" role="alert">Could not save that setting ({shellSettings.error}).</p>{/if}
    <button class="menu-item justify-between" onclick={() => { nav.themeSheetOpen = false; nav.sleepSheetOpen = true; }}>
      <span>Sleep timer</span><span class="muted">{sleepLabel(sleep.status)}</span>
    </button>
  </section>

  {#if backend}<UpdatesSection {backend} />{/if}

  <section class="settings-section px-3" aria-label="Diagnostics">
    <h3 class="group-title !px-0">Diagnostics</h3>
    <p class="muted">Pyxee keeps a log of what it does. If a song won't play or keeps skipping, the log says why.</p>
    {#if logInfo}
      <p class="well t-caption mt-2 select-all break-all" data-testid="log-path">{logInfo.file}</p>
      {#if logInfo.error}<p class="accent-text mt-1" role="alert">{logInfo.error}</p>{/if}
      <div class="mt-2 flex gap-2">
        <Button variant="primary" size="sm" onclick={() => backend?.openLogFolder().catch((e) => snackbar.push(messageOf(e)))}>Open log folder</Button>
        <Button size="sm" onclick={copyLogPath}>Copy path</Button>
      </div>
    {/if}
  </section>

  {#if backend}<AdvancedSection {backend} />{/if}
</Sheet>
