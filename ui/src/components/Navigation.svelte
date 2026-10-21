<script lang="ts">
  import { nav } from '$lib/nav.svelte';
  import { DESTINATIONS } from '$lib/destinations';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  // One list of places, two shapes. `rail`: a column on the left of a wide window (icons alone up to 1280 px, then
  // with names); the wordmark on top and Settings at the foot. `bar`: the bottom bar of a narrow window.
  let { layout }: { layout: 'rail' | 'bar' } = $props();
</script>

{#if layout === 'rail'}
  <nav class="rail" aria-label="Main" data-layout="rail">
    <div class="rail-brand">
      <Logo size={28} /><span class="wordmark rail-label">Pyxee</span>
    </div>
    <ul class="rail-list">
      {#each DESTINATIONS as it (it.route)}
        <li>
          <button class="nav-item" title={it.label} aria-current={nav.route === it.route ? 'page' : undefined} onclick={() => nav.go(it.route)}>
            <Icon name={it.icon} size={20} /><span class="rail-label">{it.label}</span>
          </button>
        </li>
      {/each}
    </ul>
    <div class="mt-auto">
      <button class="nav-item" aria-label="Settings" title="Settings" onclick={() => (nav.themeSheetOpen = true)}>
        <Icon name="settings" size={20} /><span class="rail-label" aria-hidden="true">Settings</span>
      </button>
    </div>
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
