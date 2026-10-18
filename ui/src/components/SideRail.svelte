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

<nav class="flex w-52 shrink-0 flex-col gap-0.5 border-r border-line bg-surface py-5" aria-label="Main">
  <div class="flex items-center gap-2.5 px-5 pb-6 text-[15px] font-semibold uppercase tracking-[0.2em]">
    <Logo size={26} />Pyxee
  </div>
  {#each items as it (it.route)}
    <button
      class="relative flex h-10 items-center gap-3.5 px-5 text-[14px] {nav.route === it.route ? 'bg-surface-2 font-semibold text-fg' : 'text-muted hover:bg-surface-2/60 hover:text-fg'}"
      aria-current={nav.route === it.route ? 'page' : undefined}
      onclick={() => nav.go(it.route)}
    >
      {#if nav.route === it.route}<span class="absolute inset-y-1.5 left-0 w-[3px] rounded-r bg-accent" aria-hidden="true"></span>{/if}
      <Icon name={it.icon} size={20} />{it.label}
    </button>
  {/each}
</nav>
