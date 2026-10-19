<script lang="ts">
  import { nav, type Route } from '$lib/nav.svelte';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  const items: { route: Route; label: string; icon: string }[] = [
    { route: 'home', label: 'Home', icon: 'home' },
    { route: 'explore', label: 'Explore', icon: 'explore' },
    { route: 'library', label: 'Library', icon: 'library' },
    { route: 'queue', label: 'Queue', icon: 'queue' },
  ];
</script>

<!-- The wide layout's navigation: the wordmark and four tabs along the top, search and settings at the far end. -->
<nav class="flex shrink-0 items-stretch gap-1" aria-label="Main">
  <div class="mr-5 flex items-center gap-3">
    <Logo size={26} /><span class="wordmark">Pyxee</span>
  </div>
  {#each items as it (it.route)}
    <button
      class="relative flex h-14 items-center gap-2 px-3 text-[14px] {nav.route === it.route ? 'font-semibold text-fg' : 'text-muted hover:text-fg'}"
      aria-current={nav.route === it.route ? 'page' : undefined}
      onclick={() => nav.go(it.route)}
    >
      <Icon name={it.icon} size={18} />{it.label}
      {#if nav.route === it.route}<span class="absolute inset-x-3 bottom-0 h-[3px] rounded-t bg-accent" aria-hidden="true"></span>{/if}
    </button>
  {/each}
</nav>
