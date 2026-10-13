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

<nav class="flex w-52 shrink-0 flex-col gap-1.5 border-r border-line px-3 py-5" aria-label="Main">
  <div class="flex items-center gap-2.5 px-4 pb-5 text-xl font-bold">
    <Logo size={28} />Pyxee
  </div>
  {#each items as it (it.route)}
    <button
      class="flex h-11 items-center gap-3.5 rounded-xl px-4 text-[15px] {nav.route === it.route ? 'bg-surface-2 text-fg' : 'text-muted hover:text-fg'}"
      aria-current={nav.route === it.route ? 'page' : undefined}
      onclick={() => nav.go(it.route)}
    >
      <Icon name={it.icon} />{it.label}
    </button>
  {/each}
</nav>
