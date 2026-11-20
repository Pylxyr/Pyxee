<script lang="ts">
  import Icon from './Icon.svelte';
  let { icon, title, text }: { icon: string; title: string; text: string } = $props();
</script>

<div class="flex flex-col items-center gap-2 px-6 py-16 text-center">
  <div class="mb-2 flex h-16 w-16 items-center justify-center r-full" style="background:var(--selected);color:var(--accent-ink);box-shadow:0 0 40px -12px var(--glow)"><Icon name={icon} size={28} /></div>
  <h2 class="t-subtitle">{title}</h2>
  <p class="muted max-w-sm">{text}</p>
</div>
