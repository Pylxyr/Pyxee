<script lang="ts">
  import Icon from './Icon.svelte';
  let { icon, title, text }: { icon: string; title: string; text: string } = $props();
</script>

<div class="flex flex-col items-center gap-2 px-6 py-16 text-center">
  <div class="mb-2 flex h-14 w-14 items-center justify-center r-full bg-surface-2 text-muted"><Icon name={icon} size={26} /></div>
  <h2 class="t-subtitle">{title}</h2>
  <p class="muted max-w-sm">{text}</p>
</div>
