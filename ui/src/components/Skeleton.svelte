<script lang="ts">
  let { rows = 6 }: { rows?: number } = $props();
</script>

<!-- Placeholder rows with the geometry of a track row, so nothing jumps when the real ones arrive. -->
<div aria-busy="true" aria-label="Loading">
  {#each Array(rows) as _, i (i)}
    <div class="flex items-center gap-3 px-2 py-1.5">
      <div class="w-8 shrink-0"></div>
      <div class="skeleton h-10 w-10 shrink-0"></div>
      <div class="flex-1 space-y-2">
        <div class="skeleton h-3.5 w-2/3"></div>
        <div class="skeleton h-3 w-1/3"></div>
      </div>
    </div>
  {/each}
</div>
