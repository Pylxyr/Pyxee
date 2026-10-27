<script lang="ts">
  import type { UpdateProgressDto } from '$lib/bindings/UpdateProgressDto';

  let { progress, what, testid, class: cls = '' }: {
    progress: UpdateProgressDto;
    /** What is being fetched: a version number or a tool name. */
    what: string;
    testid?: string;
    class?: string;
  } = $props();

  const mb = (n: number) => (n / (1024 * 1024)).toFixed(1);

  const known = $derived(progress.phase === 'downloading' && !!progress.total);
  const pct = $derived(known && progress.total ? Math.min(100, Math.floor((progress.received / progress.total) * 100)) : 0);
  const label = $derived(
    progress.phase === 'verifying' ? `Checking ${what}`
    : progress.phase === 'testing' ? `Testing ${what}`
    : progress.phase === 'starting' ? 'Starting the installer'
    : `Downloading ${what}`
  );
  const amount = $derived(
    progress.phase !== 'downloading' ? ''
    : progress.total ? `${mb(progress.received)} of ${mb(progress.total)} MB`
    : `${mb(progress.received)} MB`
  );
  const valuetext = $derived(known ? `${label}: ${pct}%` : `${label}…`);
</script>

<div class="uprog {cls}" data-testid={testid}>
  <div class="uprog-head">
    <span class="uprog-label" role="status" aria-live="polite">{label}{known ? '' : '…'}</span>
    {#if known}<span class="uprog-pct">{pct}%</span>{/if}
  </div>
  <div
    class="uprog-track"
    class:indeterminate={!known}
    role="progressbar"
    aria-label={label}
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={known ? pct : undefined}
    aria-valuetext={valuetext}
    style="--pct:{pct}%"
  >
    <div class="uprog-fill"></div>
  </div>
  {#if amount}<p class="uprog-amount">{amount}</p>{/if}
</div>
