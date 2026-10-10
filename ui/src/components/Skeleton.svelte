<script lang="ts">
  let { rows = 6 }: { rows?: number } = $props();
</script>

<div class="grid gap-x-6 gap-y-1 md:grid-cols-2" aria-busy="true" aria-label="Loading">
  {#each Array(rows) as _, i (i)}
    <div class="flex items-center gap-3 p-2">
      <div class="skeleton h-14 w-14 shrink-0"></div>
      <div class="flex-1 space-y-2">
        <div class="skeleton h-3.5 w-2/3"></div>
        <div class="skeleton h-3 w-1/3"></div>
      </div>
    </div>
  {/each}
</div>
