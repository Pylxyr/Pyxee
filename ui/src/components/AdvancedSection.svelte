<script lang="ts">
  import type { Backend } from '$lib/backend';
  import { messageOf } from '$lib/backend';
  import type { EnvSettingsDto } from '$lib/bindings/EnvSettingsDto';
  import { ENV_VARS } from '$lib/envvars';
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';

  let { backend }: { backend: Backend } = $props();

  let loaded = $state<EnvSettingsDto | null>(null);
  let draft = $state<Record<string, string>>({});
  let error = $state<string | null>(null);
  let saved = $state(false);
  let saving = $state(false);

  const valueOf = (s: EnvSettingsDto, name: string) => s.vars.find((v) => v.name === name)?.value ?? '';
  const fromWindows = (name: string) => loaded?.vars.find((v) => v.name === name)?.fromWindows ?? false;
  const dirty = $derived(loaded !== null && ENV_VARS.some((v) => (draft[v.name] ?? '') !== valueOf(loaded!, v.name)));

  function take(s: EnvSettingsDto) {
    loaded = s;
    draft = Object.fromEntries(ENV_VARS.map((v) => [v.name, valueOf(s, v.name)]));
  }

  // Hidden until asked for: most people never need these, so the panel starts closed and reads the file on its first opening.
  let open = $state(false);
  let requested = false;

  function toggle() {
    open = !open;
    if (open && !requested) {
      requested = true;
      backend.envGet().then(take, (e) => (error = messageOf(e)));
    }
  }

  async function save() {
    saving = true;
    error = null;
    saved = false;
    try {
      take(await backend.envSet(ENV_VARS.map((v) => ({ name: v.name, value: (draft[v.name] ?? '').trim(), fromWindows: false }))));
      saved = true;
    } catch (e) {
      error = messageOf(e);
    }
    saving = false;
  }

  const field = 'field mt-1';
</script>

<section class="settings-section px-3" aria-label="Advanced">
  <h3>
    <button class="menu-item !px-0 justify-between" aria-expanded={open} aria-controls="advanced-panel" onclick={toggle}>
      <span class="group-title !p-0">Advanced</span>
      <span class="muted inline-flex transition-transform {open ? 'rotate-180' : ''}" aria-hidden="true"><Icon name="down" size={18} /></span>
    </button>
  </h3>
  {#if open}
    <div id="advanced-panel">
    <p class="muted mt-1">
      Settings that Pyxee also reads from environment variables. They are kept in a file called .env in Pyxee's data folder and are used the next time Pyxee starts.
    </p>
    {#if loaded?.file}
      <p class="well t-caption mt-2 select-all break-all" data-testid="env-path">{loaded.file}</p>
    {:else if loaded}
      <p class="accent-text mt-2" role="alert">Pyxee has no data folder this time, so these cannot be saved.</p>
    {/if}

    {#if loaded}
      {#each ENV_VARS as v (v.name)}
        <div class="mt-3">
          <label class="t-strong block" for={`env-${v.name}`}>
            {v.title} <span class="field-mono muted font-normal">{v.name}</span>
          </label>
          <p class="muted mt-1">{v.about}</p>
          <p class="muted mt-1">{v.fill}</p>
          {#if v.choices}
            <select id={`env-${v.name}`} class={field} bind:value={draft[v.name]}>
              {#each v.choices as c (c)}<option value={c}>{c === '' ? 'Default' : c}</option>{/each}
            </select>
          {:else}
            <input id={`env-${v.name}`} class={field} placeholder={v.placeholder} spellcheck="false" autocomplete="off" bind:value={draft[v.name]} />
          {/if}
          {#if fromWindows(v.name)}
            <p class="muted mt-1">Windows also sets this variable, and that value is the one Pyxee uses.</p>
          {/if}
        </div>
      {/each}
      {#if error}<p class="accent-text mt-3" role="alert">{error}</p>{/if}
      <div class="mt-3 flex items-center gap-3">
        <Button variant="primary" disabled={!dirty || saving || !loaded.file} onclick={save}>Save</Button>
        {#if saved && !dirty}<span class="muted" role="status">Saved. Restart Pyxee to use the changes.</span>{/if}
      </div>
    {:else if error}
      <p class="accent-text mt-2" role="alert">{error}</p>
    {/if}
    </div>
  {/if}
</section>
