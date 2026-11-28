<script lang="ts">
  import { untrack } from 'svelte';
  import { nav } from '$lib/nav.svelte';
  import { DESTINATIONS } from '$lib/destinations';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  // One list of places, two shapes. `rail`: the dock beside the page of a wide window, the logo, the four places and Settings. `bar`: the bottom bar of a narrow window.
  let { layout }: { layout: 'rail' | 'bar' } = $props();

  // The pill behind the chosen place slides to the next one. It is placed once without animating (so it does not
  // sweep in from the corner on load) and animates from then on; on a page that is no place (a search) it fades out.
  // Every place is the same height, so the pill only moves (a transform), it is never resized.
  let list = $state<HTMLElement>();
  let bar = $state({ y: 0, shown: false, armed: false });
  $effect(() => {
    void nav.route;
    const el = list?.querySelector<HTMLElement>('[aria-current="page"]');
    untrack(() => {
      if (!el) {
        bar.shown = false;
        return;
      }
      bar.y = el.offsetTop;
      bar.shown = true;
      if (!bar.armed) requestAnimationFrame(() => (bar.armed = true));
    });
  });
</script>

{#if layout === 'rail'}
  <!-- The dock: the logo, the four places (icon over label) and Settings, in one rounded column beside the page. -->
  <nav class="rail" aria-label="Main" data-layout="rail">
    <button class="rail-brand" title="Pyxee" aria-label="Pyxee, home" onclick={() => nav.go('home')}><Logo size={30} /></button>
    <ul class="rail-list" bind:this={list}>
      {#each DESTINATIONS as it (it.route)}
        <li>
          <button class="rail-item" title={it.label} data-toss-dest={it.route === 'queue' ? '' : undefined} aria-current={nav.route === it.route ? 'page' : undefined} onclick={() => nav.go(it.route)}>
            <span class="rail-icon"><Icon name={it.icon} size={22} /></span><span class="rail-label">{it.label}</span>
          </button>
        </li>
      {/each}
      <li class="rail-indicator" aria-hidden="true" data-shown={bar.shown} data-armed={bar.armed} style="--ind-y:{bar.y}px"></li>
    </ul>
    <button class="rail-item rail-settings" title="Settings" onclick={() => (nav.themeSheetOpen = true)}>
      <span class="rail-icon"><Icon name="settings" size={22} /></span><span class="rail-label">Settings</span>
    </button>
  </nav>
{:else}
  <nav class="bar-nav" aria-label="Main" data-layout="bar">
    {#each DESTINATIONS as it (it.route)}
      <button class="bar-item" data-toss-dest={it.route === 'queue' ? '' : undefined} aria-current={nav.route === it.route ? 'page' : undefined} onclick={() => nav.go(it.route)}>
        <span class="bar-pill"><Icon name={it.icon} size={22} /></span>{it.label}
      </button>
    {/each}
  </nav>
{/if}
