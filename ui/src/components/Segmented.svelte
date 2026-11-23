<script lang="ts" generics="T extends string">
  import { hscroll, keepInRow } from '$lib/scrollrow';
  interface Item {
    id: T;
    label: string;
    disabled?: boolean;
    title?: string;
  }

  // One control for two jobs: `tabs` switch between panels of a page (role tab), `toggle` picks a filter (pressed button).
  let { items, value, onselect, label, mode = 'tabs', class: extra = '' }: {
    items: Item[];
    value: T;
    onselect: (id: T) => void;
    label: string;
    mode?: 'tabs' | 'toggle';
    class?: string;
  } = $props();

  // A narrow window scrolls the row: keep the chosen one in view.
  let row = $state<HTMLElement>();
  $effect(() => {
    void value;
    if (row) keepInRow(row, row.querySelector<HTMLElement>('[aria-selected="true"], [aria-pressed="true"]'), 56);
  });
</script>

<div class="segmented {extra}" role={mode === 'tabs' ? 'tablist' : 'group'} aria-label={label} bind:this={row} use:hscroll>
  {#each items as it (it.id)}
    {#if mode === 'tabs'}
      <button role="tab" class="segment" aria-selected={value === it.id} disabled={it.disabled} title={it.title} onclick={() => onselect(it.id)}>{it.label}</button>
    {:else}
      <button class="segment" aria-pressed={value === it.id} disabled={it.disabled} title={it.title} onclick={() => onselect(it.id)}>{it.label}</button>
    {/if}
  {/each}
</div>
