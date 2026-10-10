<script lang="ts">
  import Icon from './Icon.svelte';
  let { icon, title, text }: { icon: string; title: string; text: string } = $props();
</script>

<div class="flex flex-col items-center gap-2 px-6 py-16 text-center">
  <div class="mb-1 flex h-14 w-14 items-center justify-center rounded-full bg-surface-2 text-muted"><Icon name={icon} size={26} /></div>
  <h2 class="text-lg font-semibold">{title}</h2>
  <p class="max-w-sm text-sm text-muted">{text}</p>
</div>
