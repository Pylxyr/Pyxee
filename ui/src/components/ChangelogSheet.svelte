<script lang="ts">
  import type { Backend } from '$lib/backend';
  import { isVersionHeading, parseChangelog } from '$lib/changelog';
  import { updates } from '$lib/updates.svelte';
  import Button from './Button.svelte';
  import Sheet from './Sheet.svelte';
  import UpdateProgress from './UpdateProgress.svelte';

  let { backend }: { backend: Backend | null } = $props();

  const info = $derived(updates.info);
  const blocks = $derived(parseChangelog(info?.notes ?? ''));
  const installing = $derived(updates.phase === 'installing');
  /** How many releases the notes cover, when the update spans more than one (the notes then have a heading each). */
  const releaseCount = $derived(blocks.filter((b) => b.kind === 'heading' && isVersionHeading(b.text)).length);
</script>

<Sheet open={updates.changelogOpen && !!info?.available} title="What's new" onclose={() => updates.closeChangelog()}>
  {#if info}
    <p class="t-strong px-3" data-testid="changelog-version">Pyxee {info.latest} is available</p>
    <p class="muted mt-1 px-3">
      You have {info.current}.{#if releaseCount > 1} Below is what changed in each of the {releaseCount} releases since.{/if}
    </p>

    <div class="well mt-3" data-testid="changelog-notes">
      {#if blocks.length}
        {#each blocks as b, i (i)}
          {#if b.kind === 'heading' && isVersionHeading(b.text)}
            <h3 class="changelog-release {i > 0 ? 'changelog-release-next' : ''}" data-testid="changelog-release">Pyxee {b.text}</h3>
          {:else if b.kind === 'heading'}
            <h3 class="t-strong {i > 0 ? 'mt-3' : ''}">{b.text}</h3>
          {:else if b.kind === 'list'}
            <ul class="mt-1 list-disc space-y-1 pl-5">
              {#each b.items as item, j (j)}<li>{item}</li>{/each}
            </ul>
          {:else}
            <p class="mt-1">{b.text}</p>
          {/if}
        {/each}
      {:else}
        <p class="muted">This release has no written changelog.</p>
      {/if}
    </div>

    {#if installing && updates.progress}
      <UpdateProgress class="mt-3" progress={updates.progress} what={info.latest} testid="changelog-progress" />
    {:else if installing}
      <p class="muted mt-3" role="status">Downloading and checking the installer. Pyxee closes when it starts.</p>
    {/if}

    {#if info.portable}
      <p class="t-caption muted mt-3 break-all px-3">This is a portable copy. Download the new zip from {info.notesUrl} and unpack it over this folder; your data folder stays.</p>
    {:else if !info.installable}
      <p class="t-caption muted mt-3 break-all px-3">This release has no installer with a checksum list attached. Download it from {info.notesUrl}</p>
    {:else}
      <p class="t-caption muted mt-3 break-all px-3">Full release page: {info.notesUrl}</p>
    {/if}
    {#if updates.error}<p class="accent-text mt-2 px-3" role="alert">{updates.error}</p>{/if}

    <div class="mt-3 flex flex-wrap justify-end gap-2">
      <Button size="sm" onclick={() => updates.closeChangelog()}>Later</Button>
      {#if info.installable && backend}
        <Button size="sm" variant="primary" disabled={installing} onclick={() => updates.install(backend)}>Download and install {info.latest}</Button>
      {/if}
    </div>
  {/if}
</Sheet>
