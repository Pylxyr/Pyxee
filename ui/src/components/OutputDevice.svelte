<script lang="ts">
  import { onMount } from 'svelte';
  import { shellSettings } from '$lib/shell.svelte';

  // The list is asked for each time the section opens: a device plugged in meanwhile is there.
  onMount(() => {
    void shellSettings.loadDevices();
  });
  const list = $derived(shellSettings.devices);
  const missing = $derived(!!list?.selected && !list.devices.some((d) => d.id === list.selected));
</script>

{#if list && list.devices.length > 0}
  <label class="setting-hint block px-3">
    Output device
    <select
      class="field mt-1"
      aria-label="Output device"
      value={list.selected ?? ''}
      onchange={(e) => void shellSettings.chooseDevice(e.currentTarget.value || null)}
    >
      <option value="">Follow the system default</option>
      {#each list.devices as d (d.id)}<option value={d.id}>{d.name}{d.isDefault ? ' (system default)' : ''}</option>{/each}
      {#if missing}<option value={list.selected ?? ''} disabled>The chosen device (not connected)</option>{/if}
    </select>
    <span class="mt-1 block">Pyxee plays on the device chosen here. If it is unplugged, Pyxee plays on the system default until it is back, and moves over by itself when it returns. Which devices you have stays on this computer.</span>
  </label>
{/if}
