<script lang="ts">
  import { messageOf, type Backend } from '$lib/backend';
  import type { LogInfoDto } from '$lib/bindings/LogInfoDto';
  import { nav, type SettingsTab } from '$lib/nav.svelte';
  import { theme } from '$lib/theme.svelte';
  import { sleep } from '$lib/sleep.svelte';
  import { sleepLabel } from '$lib/lyrics';
  import { shellSettings } from '$lib/shell.svelte';
  import { CROSSFADE_SECONDS, prefs } from '$lib/prefs.svelte';
  import { UI_SCALES, type UiScale } from '$lib/uiscale';
  import { hasOwnWindow } from '$lib/windowControls';
  import { platform } from '$lib/platform.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';
  import IconButton from './IconButton.svelte';
  import EqSection from './EqSection.svelte';
  import LookSection from './LookSection.svelte';
  import Sheet from './Sheet.svelte';
  import ThemePicker from './ThemePicker.svelte';
  import Toggle from './Toggle.svelte';
  import UpdatesSection from './UpdatesSection.svelte';

  let { backend }: { backend: Backend | null } = $props();
  let logInfo = $state<LogInfoDto | null>(null);

  // The log location is looked up when the sheet opens. Logging is off by default: the Diagnostics section is only there when it is on.
  $effect(() => {
    if (nav.themeSheetOpen && backend) void backend.logInfo().then((i) => (logInfo = i));
  });

  // Settings is a panel with a list of categories on its side (above it in a narrow window); one category shows at a time.
  const TABS: { id: SettingsTab; label: string; icon: string; blurb: string }[] = [
    { id: 'appearance', label: 'Appearance', icon: 'palette', blurb: 'Theme, colors and size' },
    { id: 'layout', label: 'Layout', icon: 'layout', blurb: 'Corners, surfaces, panels and the home page' },
    { id: 'playback', label: 'Playback', icon: 'music', blurb: 'Crossfade between songs and the equalizer' },
    { id: 'system', label: platform.os === 'windows' ? 'Windows' : 'System', icon: 'monitor', blurb: 'Tray, notifications and sleep timer' },
    { id: 'connections', label: 'Connections', icon: 'plug', blurb: 'Discord and Last.fm' },
    { id: 'updates', label: 'Updates', icon: 'refresh', blurb: 'Pyxee and its tools' },
    { id: 'diagnostics', label: 'Diagnostics', icon: 'bug', blurb: 'The log file' },
  ];
  const tabs = $derived(TABS.filter((t) => (t.id === 'updates' ? backend !== null : t.id === 'diagnostics' ? logInfo?.enabled === true : true)));
  const tab = $derived(tabs.find((t) => t.id === nav.settingsTab) ?? tabs[0]!);

  /** Arrow keys walk the categories, as they do in a tab list. */
  function navKeys(e: KeyboardEvent) {
    const horizontal = matchMedia('(max-width: 719px)').matches;
    const next = e.key === (horizontal ? 'ArrowRight' : 'ArrowDown') ? 1 : e.key === (horizontal ? 'ArrowLeft' : 'ArrowUp') ? -1 : 0;
    if (next === 0) return;
    e.preventDefault();
    const at = tabs.findIndex((t) => t.id === tab.id);
    const to = tabs[(at + next + tabs.length) % tabs.length]!;
    nav.settingsTab = to.id;
    void Promise.resolve().then(() => document.getElementById(`settings-tab-${to.id}`)?.focus());
  }

  async function copyLogPath() {
    try {
      await navigator.clipboard.writeText(logInfo?.file ?? '');
      snackbar.push('Log file path copied');
    } catch {
      snackbar.push('Could not copy. Select the path and copy it by hand.');
    }
  }
</script>

<Sheet open={nav.themeSheetOpen} title="Settings" side="dialog" onclose={() => (nav.themeSheetOpen = false)}>
  <div class="settings">
    <nav class="settings-nav" aria-label="Settings categories">
      <h2 class="settings-nav-title">Settings</h2>
      <div class="settings-tabs" role="tablist" aria-orientation="vertical" tabindex="-1" onkeydown={navKeys}>
        {#each tabs as t (t.id)}
          <button
            id="settings-tab-{t.id}"
            class="settings-tab"
            role="tab"
            aria-selected={t.id === tab.id}
            aria-controls="settings-pane"
            tabindex={t.id === tab.id ? 0 : -1}
            onclick={() => (nav.settingsTab = t.id)}
          ><Icon name={t.icon} size={18} /><span>{t.label}</span></button>
        {/each}
      </div>
    </nav>

    <div class="settings-main">
      <header class="settings-head">
        <div class="min-w-0">
          <h2 class="t-title" data-testid="settings-heading">{tab.label}</h2>
          <p class="muted settings-blurb">{tab.blurb}</p>
        </div>
        <IconButton icon="close" label="Close Settings" onclick={() => (nav.themeSheetOpen = false)} />
      </header>

      <div id="settings-pane" class="settings-pane" role="tabpanel" aria-labelledby="settings-tab-{tab.id}" data-scroll>
        {#if tab.id === 'appearance'}
          <section class="settings-section" aria-label="Appearance">
            <ThemePicker />
            <Toggle
              label="Color from album art"
              hint="The cover of the song that is playing becomes the whole theme: background, panels, text and accent. The themes above are off while this is on."
              checked={theme.settings.dynamicColor}
              onchange={(on) => theme.update({ dynamicColor: on })}
            />
            {#if hasOwnWindow()}
              <label class="setting-hint mt-3 block px-3">
                Interface size
                <select class="field mt-1" aria-label="Interface size" value={prefs.uiScale} onchange={(e) => prefs.setUiScale(e.currentTarget.value as UiScale)}>
                  {#each UI_SCALES as s (s.id)}<option value={s.id}>{s.label}</option>{/each}
                </select>
                <span class="mt-1 block">Automatic enlarges a very large, unscaled screen (such as a 4K monitor at 100%) and leaves the rest alone.</span>
              </label>
            {/if}
          </section>
        {:else if tab.id === 'layout'}
          <LookSection />
        {:else if tab.id === 'playback'}
          <section class="settings-section" aria-label="Playback">
            <label class="setting-hint block px-3">
              Crossfade between songs
              <select class="field mt-1" aria-label="Crossfade between songs" value={String(prefs.crossfade)} onchange={(e) => prefs.setCrossfade(Number(e.currentTarget.value))}>
                {#each CROSSFADE_SECONDS as s (s)}<option value={String(s)}>{s === 0 ? 'Off' : `${s} seconds`}</option>{/each}
              </select>
              <span class="mt-1 block">One song fades out while the next fades in. It only happens when a song ends by itself: Next, Previous, seeking and stopping stay instant. A song whose length is unknown plays without it. Off keeps songs joined without a gap.</span>
            </label>
          </section>
          <EqSection />
        {:else if tab.id === 'system'}
          <section class="settings-section" aria-label={tab.label}>
            <Toggle
              label="Keep playing in the tray when the window is closed"
              hint={platform.os === 'linux' ? 'Needs a tray on your desktop: KDE Plasma has one, GNOME needs the "AppIndicator and KStatusNotifierItem Support" extension. Without a tray, closing the window leaves no way back to it except starting Pyxee again.' : undefined}
              checked={shellSettings.settings.closeToTray} onchange={(on) => shellSettings.update({ closeToTray: on })} />
            <Toggle label={platform.os === 'windows' ? 'Start Pyxee when I sign in to Windows' : 'Start Pyxee when I sign in'} checked={shellSettings.settings.startWithWindows} onchange={(on) => shellSettings.update({ startWithWindows: on })} />
            <Toggle label="Show a notification when the song changes" checked={shellSettings.settings.toastOnChange} onchange={(on) => shellSettings.update({ toastOnChange: on })} />
            {#if platform.os === 'windows'}
              <Toggle
                label="Show previous, play and next buttons in the notification area"
                hint="They sit beside the Pyxee icon, and the song title shows when you point at it. Windows may put new icons in the hidden-icons flyout (the ^ arrow): drag them out onto the taskbar once and Windows remembers."
                checked={shellSettings.settings.trayControls}
                onchange={(on) => shellSettings.update({ trayControls: on })}
              />
              <Toggle label="Clicking the Pyxee tray icon opens the mini controls (otherwise it opens the window)" checked={shellSettings.settings.flyoutOnClick} onchange={(on) => shellSettings.update({ flyoutOnClick: on })} />
            {/if}
            {#if shellSettings.error}<p class="accent-text px-3" role="alert">Could not save that setting ({shellSettings.error}).</p>{/if}
            <button class="menu-item justify-between" onclick={() => { nav.themeSheetOpen = false; nav.sleepSheetOpen = true; }}>
              <span>Sleep timer</span><span class="muted">{sleepLabel(sleep.status)}</span>
            </button>
          </section>
        {:else if tab.id === 'connections'}
          <section class="settings-section" aria-label="Connections">
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
            {#if shellSettings.error}<p class="accent-text px-3" role="alert">Could not save that setting ({shellSettings.error}).</p>{/if}
          </section>
        {:else if tab.id === 'updates'}
          {#if backend}<UpdatesSection {backend} />{/if}
        {:else if tab.id === 'diagnostics' && logInfo?.enabled}
          <section class="settings-section px-3" aria-label="Diagnostics">
            <p class="muted">Pyxee keeps a log of what it does. If a song won't play or keeps skipping, the log says why.</p>
            <p class="well t-caption mt-2 select-all break-all" data-testid="log-path">{logInfo.file}</p>
            {#if logInfo.error}<p class="accent-text mt-1" role="alert">{logInfo.error}</p>{/if}
            <div class="mt-2 flex gap-2">
              <Button variant="primary" size="sm" onclick={() => backend?.openLogFolder().catch((e) => snackbar.push(messageOf(e)))}>Open log folder</Button>
              <Button size="sm" onclick={copyLogPath}>Copy path</Button>
            </div>
          </section>
        {/if}
      </div>
    </div>
  </div>
</Sheet>
