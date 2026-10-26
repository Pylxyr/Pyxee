<script lang="ts">
  import { theme } from '$lib/theme.svelte';
  import { nav } from '$lib/nav.svelte';
  import type { ThemeDef } from '$lib/themes';
  import Button from './Button.svelte';
  import Toggle from './Toggle.svelte';

  const lights = $derived(theme.all.filter((t) => t.scheme === 'light'));
  const darks = $derived(theme.all.filter((t) => t.scheme === 'dark'));
  const active = $derived(theme.selectedId);
  const groups = $derived([
    { label: 'Dark', items: darks },
    { label: 'Light', items: lights },
  ]);

  function swatches(t: ThemeDef): string[] {
    return [t.colors.bg, t.colors.surface2, t.colors.fg, t.colors.accent];
  }
</script>

<!-- With "Color from album art" on, the cover is the theme, so these are switched off (inert: not clickable or focusable). -->
<div class="theme-picker" inert={theme.settings.dynamicColor}>
<Toggle
  label="Match system"
  hint="Use one theme when Windows is light and another when it is dark."
  checked={theme.settings.followSystem}
  onchange={(on) => theme.update({ followSystem: on })}
/>

{#if theme.settings.followSystem}
  <div class="grid grid-cols-2 gap-3 px-3 pb-2">
    <label class="setting-hint">
      When light
      <select class="field mt-1" aria-label="Light theme" value={theme.settings.lightId} onchange={(e) => theme.update({ lightId: e.currentTarget.value })}>
        {#each lights as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
      </select>
    </label>
    <label class="setting-hint">
      When dark
      <select class="field mt-1" aria-label="Dark theme" value={theme.settings.darkId} onchange={(e) => theme.update({ darkId: e.currentTarget.value })}>
        {#each darks as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
      </select>
    </label>
  </div>
{/if}

<div class="grid grid-cols-2 gap-2 px-2 py-2" role="group" aria-label="Theme">
  {#each groups as g (g.label)}
    {#if g.items.length}<p class="group-title col-span-2 !px-1 pt-2">{g.label}</p>{/if}
    {#each g.items as t (t.id)}
      <button class="theme-chip" aria-label={t.name} aria-pressed={active === t.id} title={t.description ?? 'Custom theme'} onclick={() => theme.select(t.id)}>
        <span class="flex shrink-0 overflow-hidden r-full border border-line" aria-hidden="true">
          {#each swatches(t) as c}<span class="h-5 w-2.5" style="background:{c}"></span>{/each}
        </span>
        <span class="min-w-0 leading-tight">{t.name}</span>
      </button>
    {/each}
  {/each}
</div>

<div class="flex gap-2 px-2 pb-2">
  <Button variant="primary" onclick={() => nav.openThemeEditor()}>Customize…</Button>
</div>
</div>
