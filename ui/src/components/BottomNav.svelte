<script lang="ts">
  import { nav, type Route } from '$lib/nav.svelte';
  import Icon from './Icon.svelte';

  const items: { route: Route; label: string; icon: string }[] = [
    { route: 'home', label: 'Home', icon: 'home' },
    { route: 'explore', label: 'Explore', icon: 'explore' },
    { route: 'library', label: 'Library', icon: 'library' },
    { route: 'queue', label: 'Queue', icon: 'queue' },
  ];
</script>

<nav class="flex shrink-0 border-t border-line bg-surface" aria-label="Main">
  {#each items as it (it.route)}
    <button
      class="-mt-px flex h-14 flex-1 flex-col items-center justify-center gap-0.5 border-t-2 text-[11px] {nav.route === it.route ? 'border-accent text-fg' : 'border-transparent text-muted'}"
      aria-current={nav.route === it.route ? 'page' : undefined}
      onclick={() => nav.go(it.route)}
    >
      <Icon name={it.icon} size={22} />{it.label}
    </button>
  {/each}
</nav>
