<script lang="ts">
  import { nav } from '$lib/nav.svelte';
  import { sleep } from '$lib/sleep.svelte';
  import { sleepLabel } from '$lib/lyrics';
  import { snackbar } from '$lib/snackbar.svelte';
  import type { SleepSettingDto } from '$lib/bindings/SleepSettingDto';
  import Button from './Button.svelte';
  import MenuItem from './MenuItem.svelte';
  import Sheet from './Sheet.svelte';

  const options: { label: string; setting: SleepSettingDto }[] = [
    { label: '15 minutes', setting: { kind: 'minutes', minutes: 15 } },
    { label: '30 minutes', setting: { kind: 'minutes', minutes: 30 } },
    { label: '45 minutes', setting: { kind: 'minutes', minutes: 45 } },
    { label: '1 hour', setting: { kind: 'minutes', minutes: 60 } },
    { label: '90 minutes', setting: { kind: 'minutes', minutes: 90 } },
    { label: 'End of this song', setting: { kind: 'endOfTrack' } },
    { label: 'After 2 songs', setting: { kind: 'tracks', count: 2 } },
    { label: 'After 3 songs', setting: { kind: 'tracks', count: 3 } },
  ];

  function choose(o: { label: string; setting: SleepSettingDto }) {
    void sleep.set(o.setting);
    nav.sleepSheetOpen = false;
    snackbar.push(`Sleep timer: ${o.label.toLowerCase()}`, { action: { label: 'Turn off', run: () => void sleep.cancel() } });
  }
</script>

<Sheet open={nav.sleepSheetOpen} title="Sleep timer" onclose={() => (nav.sleepSheetOpen = false)}>
  {#if sleep.status}
    <div class="well mb-2 flex items-center justify-between" role="status">
      <span class="t-strong" data-testid="sleep-status">{sleepLabel(sleep.status)}</span>
      <span class="flex gap-2">
        {#if sleep.status.kind === 'minutes'}
          <Button size="sm" onclick={() => sleep.extend(15)}>+15 min</Button>
        {/if}
        <Button size="sm" variant="primary" onclick={() => sleep.cancel()}>Turn off</Button>
      </span>
    </div>
  {/if}
  <p class="setting-hint px-3 pb-2">Playback pauses by itself. A timer in minutes fades the volume out over its last 15 seconds.</p>
  {#each options as o (o.label)}
    <MenuItem onclick={() => choose(o)}>{o.label}</MenuItem>
  {/each}
</Sheet>
