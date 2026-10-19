<script lang="ts">
  import type { Backend } from '$lib/backend';
  import { messageOf } from '$lib/backend';
  import type { EnvSettingsDto } from '$lib/bindings/EnvSettingsDto';
  import { ENV_VARS } from '$lib/envvars';
  import { onMount } from 'svelte';

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

  onMount(() => {
    backend.envGet().then(take, (e) => (error = messageOf(e)));
  });

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

  const field = 'mt-1 w-full rounded-lg bg-surface px-3 py-2 text-sm text-fg';
</script>

<section class="mt-3 border-t border-line px-3 pt-3" aria-label="Advanced">
  <h3 class="text-sm font-semibold">Advanced</h3>
  <p class="mt-1 text-[13px] text-muted">
    Settings that Pyxee also reads from environment variables. They are kept in a file called .env in Pyxee's data folder and are used the next time Pyxee starts.
  </p>
  {#if loaded?.file}
    <p class="mt-2 select-all break-all rounded-lg bg-surface px-3 py-2 text-[12px]" data-testid="env-path">{loaded.file}</p>
  {:else if loaded}
    <p class="mt-2 text-[13px] text-accent" role="alert">Pyxee has no data folder this time, so these cannot be saved.</p>
  {/if}

  {#if loaded}
    {#each ENV_VARS as v (v.name)}
      <div class="mt-3">
        <label class="block text-[14px] font-medium" for={`env-${v.name}`}>
          {v.title} <span class="font-mono text-[12px] font-normal text-muted">{v.name}</span>
        </label>
        <p class="mt-1 text-[13px] text-muted">{v.about}</p>
        <p class="mt-1 text-[13px] text-muted">{v.fill}</p>
        {#if v.choices}
          <select id={`env-${v.name}`} class={field} bind:value={draft[v.name]}>
            {#each v.choices as c (c)}<option value={c}>{c === '' ? 'Default' : c}</option>{/each}
          </select>
        {:else}
          <input id={`env-${v.name}`} class={field} placeholder={v.placeholder} spellcheck="false" autocomplete="off" bind:value={draft[v.name]} />
        {/if}
        {#if fromWindows(v.name)}
          <p class="mt-1 text-[13px] text-muted">Windows also sets this variable, and that value is the one Pyxee uses.</p>
        {/if}
      </div>
    {/each}
    {#if error}<p class="mt-3 text-[13px] text-accent" role="alert">{error}</p>{/if}
    <div class="mt-3 flex items-center gap-3">
      <button class="rounded-md bg-fg px-4 py-1.5 text-sm font-semibold text-on-fg disabled:opacity-40" disabled={!dirty || saving || !loaded.file} onclick={save}>Save</button>
      {#if saved && !dirty}<span class="text-[13px] text-muted" role="status">Saved. Restart Pyxee to use the changes.</span>{/if}
    </div>
  {:else if error}
    <p class="mt-2 text-[13px] text-accent" role="alert">{error}</p>
  {/if}
</section>
