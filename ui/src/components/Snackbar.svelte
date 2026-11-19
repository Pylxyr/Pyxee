<script lang="ts">
  import { fly } from 'svelte/transition';
  import { backOut } from 'svelte/easing';
  import { snackbar } from '$lib/snackbar.svelte';
  import { MOTION, motionMs, motionPx } from '$lib/motion';
  import { toastRail } from '$lib/rail';
</script>

<!-- Above the dock and the bottom bar in a narrow window; at the foot of the page in a wide one. -->
<div class="pointer-events-none fixed inset-x-0 bottom-36 z-[1000] flex flex-col items-center gap-2 px-4 min-[900px]:bottom-6" aria-live="polite">
  {#each snackbar.items as s (s.id)}
    <div
      transition:fly={{ y: motionPx(20), duration: motionMs(MOTION.base + 60), easing: backOut }}
      class="toast pointer-events-auto flex w-full max-w-md items-center gap-4 bg-fg px-4 py-3 text-on-fg"
      role="status"
      onmouseenter={() => snackbar.pause(s.id)}
      onmouseleave={() => snackbar.resume(s.id)}
      onfocusin={() => snackbar.pause(s.id)}
      onfocusout={() => snackbar.resume(s.id)}
    >
      <span class="flex-1">{s.message}</span>
      {#if s.action}
        <button class="btn-link" style="color:var(--on-fg)" onclick={() => snackbar.act(s.id)}>
          {s.action.label}
        </button>
      {/if}
      {#if snackbar.rails[s.id]}<span class="toast-rail" aria-hidden="true" use:toastRail={snackbar.rails[s.id]!}></span>{/if}
    </div>
  {/each}
</div>
