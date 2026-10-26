<script lang="ts">
  import { nav } from '$lib/nav.svelte';
  import { DESTINATIONS } from '$lib/destinations';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  // One list of places, two shapes. `tabs`: the top of a wide window, the wordmark then the four places as tabs.
  // `bar`: the bottom bar of a narrow window.
  let { layout }: { layout: 'tabs' | 'bar' } = $props();
</script>

{#if layout === 'tabs'}
  <nav class="tabs-nav" aria-label="Main" data-layout="tabs">
    <div class="tabs-brand">
      <Logo size={24} /><span class="wordmark">Pyxee</span>
    </div>
    <ul class="tabs-list">
      {#each DESTINATIONS as it (it.route)}
        <li>
          <button class="tab-item" aria-current={nav.route === it.route ? 'page' : undefined} onclick={() => nav.go(it.route)}>{it.label}</button>
        </li>
      {/each}
    </ul>
  </nav>
{:else}
  <nav class="bar-nav" aria-label="Main" data-layout="bar">
    {#each DESTINATIONS as it (it.route)}
      <button class="bar-item" aria-current={nav.route === it.route ? 'page' : undefined} onclick={() => nav.go(it.route)}>
        <span class="bar-pill"><Icon name={it.icon} size={22} /></span>{it.label}
      </button>
    {/each}
  </nav>
{/if}
