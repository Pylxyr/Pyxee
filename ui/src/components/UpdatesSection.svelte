<script lang="ts">
  import type { Backend } from '$lib/backend';
  import { messageOf } from '$lib/backend';
  import type { UpdateProgressDto } from '$lib/bindings/UpdateProgressDto';
  import type { YtdlpUpdateDto } from '$lib/bindings/YtdlpUpdateDto';
  import { updates } from '$lib/updates.svelte';
  import { onMount } from 'svelte';
  import Button from './Button.svelte';
  import UpdateProgress from './UpdateProgress.svelte';

  let { backend }: { backend: Backend } = $props();

  type Phase = 'idle' | 'checking' | 'installing';

  let toolPhase = $state<Phase>('idle');
  let tool = $state<YtdlpUpdateDto | null>(null);
  let toolInstalled = $state<string | null>(null);
  let toolError = $state<string | null>(null);

  let toolProgress = $state<UpdateProgressDto | null>(null);

  onMount(() => {
    let off = () => {};
    let gone = false;
    backend
      .onUpdateProgress((p) => {
        if (p.target !== 'app') toolProgress = p; // the app's own download is followed by the update store
      })
      .then((f) => (gone ? f() : (off = f)));
    return () => {
      gone = true;
      off();
    };
  });

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

</script>

<section class="settings-section px-3" aria-label="Updates">
  <h3 class="group-title !px-0">Updates</h3>

  <div class="well mt-2">
    <p class="t-strong">Pyxee</p>
    <p class="muted mt-1" role="status" data-testid="app-update-status">
      {#if updates.phase === 'checking'}Checking for a newer version…
      {:else if updates.phase === 'installing'}Downloading and checking the installer. Pyxee closes when it starts.
      {:else if updates.info && updates.info.available}Version {updates.info.latest} is available (you have {updates.info.current}).
      {:else if updates.info}You have the latest version ({updates.info.current}).
      {:else}Looks at the GitHub releases. The installer's SHA-256 is checked before it runs; it is not code-signed yet, so Windows SmartScreen may ask.{/if}
    </p>
    {#if updates.phase === 'installing' && updates.progress}
      <UpdateProgress class="mt-3" progress={updates.progress} what={updates.info?.latest ?? 'Pyxee'} testid="app-update-progress" />
    {/if}
    {#if updates.info?.available && updates.info.portable}
      <p class="t-caption muted mt-1 break-all" data-testid="portable-update">This is a portable copy. Download the new zip from {updates.info.notesUrl} and unpack it over this folder; your data folder stays.</p>
    {:else if updates.info?.available && !updates.info.installable}
      <p class="t-caption muted mt-1 break-all">This release has no installer with a checksum list attached. Download it from {updates.info.notesUrl}</p>
    {/if}
    {#if updates.error}<p class="accent-text mt-1" role="alert">{updates.error}</p>{/if}
    <div class="mt-2 flex flex-wrap gap-2">
      <Button size="sm" disabled={updates.phase !== 'idle'} onclick={() => updates.check(backend)}>Check for updates</Button>
      {#if updates.info?.available}
        <Button size="sm" disabled={updates.phase === 'installing'} onclick={() => updates.openChangelog()}>What's new</Button>
        {#if updates.info.installable}
          <Button size="sm" variant="primary" disabled={updates.phase !== 'idle'} onclick={() => updates.install(backend)}>Download and install {updates.info.latest}</Button>
        {/if}
      {/if}
    </div>
  </div>

  <div class="well mt-2">
    <p class="t-strong">yt-dlp (finds the music)</p>
    <p class="muted mt-1" role="status" data-testid="ytdlp-update-status">
      {#if toolPhase === 'checking'}Checking…
      {:else if toolPhase === 'installing'}Downloading, checking and testing the new yt-dlp…
      {:else if toolInstalled}Updated to {toolInstalled}. It is used from the next search.
      {:else if tool && tool.available}A newer yt-dlp is available{tool.currentVersion ? ` (you have ${tool.currentVersion})` : ''}.
      {:else if tool}yt-dlp is up to date{tool.currentVersion ? ` (${tool.currentVersion})` : ''}.
      {:else}Downloads are checked against yt-dlp's published SHA-256 and test-run before they replace the current one; if anything fails, nothing changes.{/if}
    </p>
    {#if toolPhase === 'installing' && toolProgress}
      <UpdateProgress class="mt-3" progress={toolProgress} what="yt-dlp" testid="ytdlp-update-progress" />
    {/if}
    {#if toolError}<p class="accent-text mt-1" role="alert">{toolError}</p>{/if}
    <div class="mt-2 flex flex-wrap gap-2">
      <Button size="sm" disabled={toolPhase !== 'idle'} onclick={checkTool}>Check yt-dlp</Button>
      {#if tool?.available && !toolInstalled}
        <Button size="sm" variant="primary" disabled={toolPhase !== 'idle'} onclick={installTool}>Update yt-dlp</Button>
      {/if}
    </div>
  </div>
</section>
