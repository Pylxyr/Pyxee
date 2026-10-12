<script lang="ts">
  import { fly } from 'svelte/transition';
  import { snackbar } from '$lib/snackbar.svelte';
</script>

<div class="pointer-events-none fixed inset-x-0 bottom-28 z-[1000] flex flex-col items-center gap-2 px-4 lg:bottom-24" aria-live="polite">
  {#each snackbar.items as s (s.id)}
    <div
      transition:fly={{ y: 16, duration: 160 }}
      class="pointer-events-auto flex w-full max-w-md items-center gap-4 rounded-lg bg-fg px-4 py-3 text-sm text-on-fg shadow-lg"
      role="status"
    >
      <span class="flex-1">{s.message}</span>
      {#if s.action}
        <button class="font-semibold uppercase tracking-wide text-accent" onclick={() => snackbar.act(s.id)}>
          {s.action.label}
        </button>
      {/if}
    </div>
  {/each}
</div>
