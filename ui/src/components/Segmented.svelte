<script lang="ts" generics="T extends string">
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
</script>

<div class="segmented {extra}" role={mode === 'tabs' ? 'tablist' : 'group'} aria-label={label}>
  {#each items as it (it.id)}
    {#if mode === 'tabs'}
      <button role="tab" class="segment" aria-selected={value === it.id} disabled={it.disabled} title={it.title} onclick={() => onselect(it.id)}>{it.label}</button>
    {:else}
      <button class="segment" aria-pressed={value === it.id} disabled={it.disabled} title={it.title} onclick={() => onselect(it.id)}>{it.label}</button>
    {/if}
  {/each}
</div>
