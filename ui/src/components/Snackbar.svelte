<script lang="ts">
  import { fly } from 'svelte/transition';
  import { snackbar } from '$lib/snackbar.svelte';
  import { MOTION, motionMs, motionPx } from '$lib/motion';
</script>

<!-- Above the dock and the bottom bar in a narrow window; at the foot of the page in a wide one. -->
<div class="pointer-events-none fixed inset-x-0 bottom-36 z-[1000] flex flex-col items-center gap-2 px-4 min-[900px]:bottom-6" aria-live="polite">
  {#each snackbar.items as s (s.id)}
    <div
      transition:fly={{ y: motionPx(16), duration: motionMs(MOTION.fast + 40) }}
      class="toast pointer-events-auto flex w-full max-w-md items-center gap-4 bg-fg px-4 py-3 text-on-fg"
      role="status"
    >
      <span class="flex-1">{s.message}</span>
      {#if s.action}
        <button class="btn-link" style="color:var(--on-fg)" onclick={() => snackbar.act(s.id)}>
          {s.action.label}
        </button>
      {/if}
    </div>
  {/each}
</div>
