<script lang="ts">
  import { theme } from '$lib/theme.svelte';
  import { nav } from '$lib/nav.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { normalizeHex } from '$lib/color';
  import { COLOR_FIELDS, contrastChecks, exportTheme, parseThemeText, type ColorKey, type ThemeDef } from '$lib/themes';
  import Button from './Button.svelte';
  import Segmented from './Segmented.svelte';
  import Sheet from './Sheet.svelte';

  const open = $derived(nav.themeEditorOpen);
  const bases: { id: 'dark' | 'light'; label: string }[] = [
    { id: 'dark', label: 'Dark base' },
    { id: 'light', label: 'Light base' },
  ];

  let draft = $state<ThemeDef>(blank());
  /** What the draft started as, for Reset. */
  let origin = $state<ThemeDef>(blank());
  let importText = $state('');
  let error = $state<string | null>(null);
  /** Hex text being typed per field (kept apart so half-typed values do not break the colour). */
  let typed = $state<Partial<Record<ColorKey, string>>>({});

  function blank(): ThemeDef {
    return structuredClone($state.snapshot(theme.active));
  }

  function begin(def: ThemeDef) {
    // A built-in theme is never edited itself: the draft is a new custom theme made from it.
    const d: ThemeDef = def.builtin ? { id: '', name: `${def.name} (custom)`, scheme: def.scheme, colors: { ...def.colors } } : { id: def.id, name: def.name, scheme: def.scheme, colors: { ...def.colors } };
    draft = d;
    origin = structuredClone(d);
    typed = {};
    error = null;
    importText = '';
  }

  // Start from the active theme each time the editor opens.
  let wasOpen = false;
  $effect(() => {
    if (open && !wasOpen) begin($state.snapshot(theme.active) as ThemeDef);
    wasOpen = open;
  });

  // Show the draft live while editing; go back to the saved theme when done.
  $effect(() => {
    if (!open) return;
    theme.preview = { id: draft.id, name: draft.name, scheme: draft.scheme, colors: { ...draft.colors } };
    return () => (theme.preview = null);
  });

  const checks = $derived(contrastChecks(draft.colors).filter((c) => !c.ok));
  const editingSaved = $derived(draft.id !== '' && theme.settings.custom.some((t) => t.id === draft.id));

  function setColor(key: ColorKey, value: string) {
    const hex = normalizeHex(value);
    if (hex) draft.colors[key] = hex;
  }

  function close() {
    nav.closeThemeEditor();
  }

  function save() {
    try {
      const saved = theme.saveCustom(draft);
      theme.select(saved.id);
      nav.closeThemeEditor();
      snackbar.push(`Saved theme “${saved.name}”`);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Could not save the theme.';
    }
  }

  function remove() {
    const name = draft.name;
    theme.deleteCustom(draft.id);
    nav.closeThemeEditor();
    snackbar.push(`Deleted theme “${name}”`);
  }

  async function copyExport() {
    try {
      await navigator.clipboard.writeText(exportTheme(draft));
      snackbar.push('Theme copied. Paste it anywhere to share it.');
    } catch {
      error = "Couldn't copy to the clipboard.";
    }
  }

  function doImport() {
    try {
      const def = parseThemeText(importText);
      draft = { ...def, id: '' };
      origin = structuredClone($state.snapshot(draft));
      typed = {};
      error = null;
      importText = '';
    } catch (e) {
      error = e instanceof Error ? e.message : 'Could not read that theme.';
    }
  }

  function reset() {
    draft = structuredClone($state.snapshot(origin));
    typed = {};
    error = null;
  }
</script>

<Sheet {open} title="Customize theme" onclose={close}>
  <div class="max-h-[70dvh] overflow-y-auto px-2" data-testid="theme-editor">
    <label class="setting-hint">
      Name
      <input class="field mt-1" aria-label="Theme name" maxlength="40" bind:value={draft.name} />
    </label>

    <Segmented class="mt-3" mode="toggle" items={bases} value={draft.scheme} onselect={(id) => (draft.scheme = id)} label="Base" />
    <p class="setting-hint mt-1">The base sets form controls, scroll bars and the window frame.</p>

    <ul class="mt-3 space-y-2" aria-label="Colors">
      {#each COLOR_FIELDS as f (f.key)}
        <li class="flex items-center gap-3">
          <input
            type="color"
            class="h-9 w-12 shrink-0 cursor-pointer r-sm border border-line bg-transparent p-0.5"
            aria-label="{f.label} color"
            value={draft.colors[f.key]}
            oninput={(e) => { setColor(f.key, e.currentTarget.value); typed[f.key] = undefined; }}
          />
          <span class="min-w-0 flex-1">
            {f.label}
            <span class="setting-hint truncate">{f.hint}</span>
          </span>
          <input
            class="field field-mono !w-24"
            aria-label="{f.label} hex"
            spellcheck="false"
            value={typed[f.key] ?? draft.colors[f.key]}
            oninput={(e) => { typed[f.key] = e.currentTarget.value; setColor(f.key, e.currentTarget.value); }}
            onblur={() => (typed[f.key] = undefined)}
          />
        </li>
      {/each}
    </ul>

    {#if checks.length > 0}
      <ul class="well t-caption mt-3" role="status" aria-label="Readability">
        {#each checks as c (c.label)}
          <li>{c.label} is hard to read ({c.ratio.toFixed(1)}:1, aim for {c.min}:1 or more).</li>
        {/each}
      </ul>
    {/if}

    <details class="well mt-3">
      <summary class="cursor-pointer">Import a theme</summary>
      <textarea class="field field-mono mt-2 h-24" aria-label="Theme text" placeholder="Paste theme text here" bind:value={importText}></textarea>
      <Button class="mt-2" disabled={!importText.trim()} onclick={doImport}>Import</Button>
    </details>

    {#if error}<p class="accent-text mt-3" role="alert">{error}</p>{/if}
  </div>

  <div class="mt-4 flex flex-wrap gap-2 px-2">
    <Button variant="primary" onclick={save}>Save</Button>
    <Button onclick={close}>Cancel</Button>
    <Button onclick={reset}>Reset</Button>
    <Button onclick={copyExport}>Export</Button>
    {#if editingSaved}
      <Button class="ml-auto" onclick={remove}><span class="accent-text">Delete</span></Button>
    {/if}
  </div>
</Sheet>
