<script lang="ts">
  import type { Backend } from '$lib/backend';
  import { messageOf } from '$lib/backend';
  import type { AppUpdateDto } from '$lib/bindings/AppUpdateDto';
  import type { UpdateProgressDto } from '$lib/bindings/UpdateProgressDto';
  import type { YtdlpUpdateDto } from '$lib/bindings/YtdlpUpdateDto';
  import { onMount } from 'svelte';

  let { backend }: { backend: Backend } = $props();

  type Phase = 'idle' | 'checking' | 'installing';
  let appPhase = $state<Phase>('idle');
  let app = $state<AppUpdateDto | null>(null);
  let appError = $state<string | null>(null);

  let toolPhase = $state<Phase>('idle');
  let tool = $state<YtdlpUpdateDto | null>(null);
  let toolInstalled = $state<string | null>(null);
  let toolError = $state<string | null>(null);

  let appProgress = $state<UpdateProgressDto | null>(null);
  let toolProgress = $state<UpdateProgressDto | null>(null);

  onMount(() => {
    let off = () => {};
    let gone = false;
    backend
      .onUpdateProgress((p) => {
        if (p.target === 'app') appProgress = p;
        else toolProgress = p;
      })
      .then((f) => (gone ? f() : (off = f)));
    return () => {
      gone = true;
      off();
    };
  });

  const mb = (n: number) => (n / (1024 * 1024)).toFixed(1);
  /** "Downloading 0.3.6: 12.3 of 18.4 MB (67%)" and the steps after it. */
  function progressText(p: UpdateProgressDto, what: string): string {
    if (p.phase === 'verifying') return `Checking ${what}…`;
    if (p.phase === 'testing') return `Testing ${what}…`;
    if (p.phase === 'starting') return 'Starting the installer…';
    if (p.total) return `Downloading ${what}: ${mb(p.received)} of ${mb(p.total)} MB (${Math.min(100, Math.floor((p.received / p.total) * 100))}%)`;
    return `Downloading ${what}: ${mb(p.received)} MB`;
  }

  async function checkApp() {
    appPhase = 'checking';
    appError = null;
    try {
      app = await backend.updateCheckApp();
    } catch (e) {
      app = null;
      appError = messageOf(e);
    }
    appPhase = 'idle';
  }

  async function installApp() {
    appPhase = 'installing';
    appError = null;
    appProgress = null;
    try {
      await backend.updateInstallApp(); // starts the installer and closes Pyxee
    } catch (e) {
      appError = messageOf(e);
    }
    appPhase = 'idle';
    appProgress = null;
  }

  async function checkTool() {
    toolPhase = 'checking';
    toolError = null;
    toolInstalled = null;
    try {
      tool = await backend.updateCheckYtdlp();
    } catch (e) {
      tool = null;
      toolError = messageOf(e);
    }
    toolPhase = 'idle';
  }

  async function installTool() {
    toolPhase = 'installing';
    toolError = null;
    toolProgress = null;
    try {
      toolInstalled = await backend.updateInstallYtdlp();
      tool = await backend.updateCheckYtdlp().catch(() => ({ available: false, currentVersion: toolInstalled }));
    } catch (e) {
      toolError = messageOf(e);
    }
    toolPhase = 'idle';
    toolProgress = null;
  }

  const btn = 'rounded-md border border-line bg-surface px-4 py-1.5 text-sm font-semibold hover:bg-surface-2 disabled:opacity-40';
  const primary = 'rounded-md bg-accent px-4 py-1.5 text-sm font-semibold text-on-accent disabled:opacity-40';
</script>

<section class="mt-3 border-t border-line px-3 pt-3" aria-label="Updates">
  <h3 class="text-sm font-semibold">Updates</h3>

  <div class="mt-2 rounded-lg bg-surface px-3 py-3">
    <p class="text-[14px] font-medium">Pyxee</p>
    <p class="mt-1 text-[13px] text-muted" role="status" data-testid="app-update-status">
      {#if appPhase === 'checking'}Checking for a newer version…
      {:else if appPhase === 'installing'}Downloading and checking the installer. Pyxee closes when it starts.
      {:else if app && app.available}Version {app.latest} is available (you have {app.current}).
      {:else if app}You have the latest version ({app.current}).
      {:else}Looks at the GitHub releases. The installer's SHA-256 is checked before it runs; it is not code-signed yet, so Windows SmartScreen may ask.{/if}
    </p>
    {#if appPhase === 'installing' && appProgress}
      <div role="status" aria-live="polite" data-testid="app-update-progress">
        {#if appProgress.phase === 'downloading' && appProgress.total}
          <progress class="mt-2 w-full" value={appProgress.received} max={appProgress.total}></progress>
        {:else}
          <progress class="mt-2 w-full"></progress>
        {/if}
        <p class="mt-1 text-[13px] text-muted">{progressText(appProgress, app?.latest ?? 'Pyxee')}</p>
      </div>
    {/if}
    {#if app?.available && app.portable}
      <p class="mt-1 break-all text-[12px] text-muted" data-testid="portable-update">This is a portable copy. Download the new zip from {app.notesUrl} and unpack it over this folder; your data folder stays.</p>
    {:else if app?.available && !app.installable}
      <p class="mt-1 break-all text-[12px] text-muted">This release has no installer with a checksum list attached. Download it from {app.notesUrl}</p>
    {/if}
    {#if appError}<p class="mt-1 text-[13px] text-accent" role="alert">{appError}</p>{/if}
    <div class="mt-2 flex flex-wrap gap-2">
      <button class={btn} disabled={appPhase !== 'idle'} onclick={checkApp}>Check for updates</button>
      {#if app?.available && app.installable}
        <button class={primary} disabled={appPhase !== 'idle'} onclick={installApp}>Download and install {app.latest}</button>
      {/if}
    </div>
  </div>

  <div class="mt-2 rounded-lg bg-surface px-3 py-3">
    <p class="text-[14px] font-medium">yt-dlp (finds the music)</p>
    <p class="mt-1 text-[13px] text-muted" role="status" data-testid="ytdlp-update-status">
      {#if toolPhase === 'checking'}Checking…
      {:else if toolPhase === 'installing'}Downloading, checking and testing the new yt-dlp…
      {:else if toolInstalled}Updated to {toolInstalled}. It is used from the next search.
      {:else if tool && tool.available}A newer yt-dlp is available{tool.currentVersion ? ` (you have ${tool.currentVersion})` : ''}.
      {:else if tool}yt-dlp is up to date{tool.currentVersion ? ` (${tool.currentVersion})` : ''}.
      {:else}Downloads are checked against yt-dlp's published SHA-256 and test-run before they replace the current one; if anything fails, nothing changes.{/if}
    </p>
    {#if toolPhase === 'installing' && toolProgress}
      <div role="status" aria-live="polite" data-testid="ytdlp-update-progress">
        {#if toolProgress.phase === 'downloading' && toolProgress.total}
          <progress class="mt-2 w-full" value={toolProgress.received} max={toolProgress.total}></progress>
        {:else}
          <progress class="mt-2 w-full"></progress>
        {/if}
        <p class="mt-1 text-[13px] text-muted">{progressText(toolProgress, 'yt-dlp')}</p>
      </div>
    {/if}
    {#if toolError}<p class="mt-1 text-[13px] text-accent" role="alert">{toolError}</p>{/if}
    <div class="mt-2 flex flex-wrap gap-2">
      <button class={btn} disabled={toolPhase !== 'idle'} onclick={checkTool}>Check yt-dlp</button>
      {#if tool?.available && !toolInstalled}
        <button class={primary} disabled={toolPhase !== 'idle'} onclick={installTool}>Update yt-dlp</button>
      {/if}
    </div>
  </div>
</section>
