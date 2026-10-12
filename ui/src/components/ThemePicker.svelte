<script lang="ts">
  import { theme } from '$lib/theme.svelte';
  import { nav } from '$lib/nav.svelte';
  import type { ThemeDef } from '$lib/themes';

  const lights = $derived(theme.all.filter((t) => t.scheme === 'light'));
  const darks = $derived(theme.all.filter((t) => t.scheme === 'dark'));
  const active = $derived(theme.selectedId);

  function swatches(t: ThemeDef): string[] {
    return [t.colors.bg, t.colors.surface2, t.colors.fg, t.colors.accent];
  }
</script>

<label class="flex items-center justify-between rounded-lg px-3 py-3 hover:bg-surface">
  <span>
    Match system
    <span class="block text-xs text-muted">Use one theme when Windows is light and another when it is dark.</span>
  </span>
  <input type="checkbox" checked={theme.settings.followSystem} onchange={(e) => theme.update({ followSystem: e.currentTarget.checked })} />
</label>

{#if theme.settings.followSystem}
  <div class="grid grid-cols-2 gap-3 px-3 pb-2">
    <label class="text-xs text-muted">
      When light
      <select class="mt-1 w-full rounded-lg bg-surface px-2 py-2 text-sm text-fg" aria-label="Light theme" value={theme.settings.lightId} onchange={(e) => theme.update({ lightId: e.currentTarget.value })}>
        {#each lights as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
      </select>
    </label>
    <label class="text-xs text-muted">
      When dark
      <select class="mt-1 w-full rounded-lg bg-surface px-2 py-2 text-sm text-fg" aria-label="Dark theme" value={theme.settings.darkId} onchange={(e) => theme.update({ darkId: e.currentTarget.value })}>
        {#each darks as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
      </select>
    </label>
  </div>
{/if}

<div class="grid grid-cols-2 gap-2 px-2 py-2" role="group" aria-label="Theme">
  {#each theme.all as t (t.id)}
    <button
      class="flex items-center gap-2.5 rounded-xl border-2 px-3 py-2.5 text-left text-sm font-medium {active === t.id ? 'border-accent bg-surface-2' : 'border-transparent bg-surface hover:bg-surface-2'}"
      aria-label={t.name}
      aria-pressed={active === t.id}
      title={t.description ?? 'Custom theme'}
      onclick={() => theme.select(t.id)}
    >
      <span class="flex shrink-0 overflow-hidden rounded-full border border-line" aria-hidden="true">
        {#each swatches(t) as c}<span class="h-5 w-3.5" style="background:{c}"></span>{/each}
      </span>
      <span class="min-w-0 truncate">{t.name}</span>
    </button>
  {/each}
</div>

<div class="flex gap-2 px-2 pb-2">
  <button class="rounded-full bg-fg px-4 py-1.5 text-sm font-semibold text-on-fg" onclick={() => nav.openThemeEditor()}>Customize…</button>
</div>
