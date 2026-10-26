<script lang="ts">
  import { untrack } from 'svelte';
  import { nav } from '$lib/nav.svelte';
  import { DESTINATIONS } from '$lib/destinations';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  // One list of places, two shapes. `tabs`: the top of a wide window, the wordmark then the four places as tabs.
  // `bar`: the bottom bar of a narrow window.
  let { layout }: { layout: 'tabs' | 'bar' } = $props();

  // The bar under the chosen tab slides to the next one. It is placed once without animating (so it does not
  // sweep in from the corner on load) and animates from then on; on a page that is no tab (a search) it fades out.
  let list = $state<HTMLElement>();
  let bar = $state({ x: 0, w: 0, shown: false, armed: false });
  $effect(() => {
    void nav.route;
    const el = list?.querySelector<HTMLElement>('[aria-current="page"]');
    untrack(() => {
      if (!el) {
        bar.shown = false;
        return;
      }
      bar.x = el.offsetLeft + 14;
      bar.w = Math.max(0, el.offsetWidth - 28);
      bar.shown = true;
      if (!bar.armed) requestAnimationFrame(() => (bar.armed = true));
    });
  });
</script>

{#if layout === 'tabs'}
  <nav class="tabs-nav" aria-label="Main" data-layout="tabs">
    <div class="tabs-brand">
      <Logo size={24} /><span class="wordmark">Pyxee</span>
    </div>
    <ul class="tabs-list" bind:this={list}>
      {#each DESTINATIONS as it (it.route)}
        <li>
          <button class="tab-item" aria-current={nav.route === it.route ? 'page' : undefined} onclick={() => nav.go(it.route)}>{it.label}</button>
        </li>
      {/each}
      <li class="tab-indicator" aria-hidden="true" data-shown={bar.shown} data-armed={bar.armed} style="--ind-x:{bar.x}px;--ind-w:{bar.w}px"></li>
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
