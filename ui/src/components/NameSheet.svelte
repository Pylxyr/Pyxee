<script lang="ts">
  import { nav } from '$lib/nav.svelte';
  import { messageOf } from '$lib/backend';
  import Sheet from './Sheet.svelte';

  let name = $state('');
  let error = $state('');
  let busy = $state(false);
  const prompt = $derived(nav.namePrompt);

  $effect(() => {
    if (prompt) {
      name = prompt.initial;
      error = '';
    }
  });

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!prompt || busy) return;
    busy = true;
    error = '';
    try {
      await prompt.run(name);
      nav.namePrompt = null;
    } catch (err) {
      error = messageOf(err);
    } finally {
      busy = false;
    }
  }
</script>

<Sheet open={prompt !== null} title={prompt?.title ?? ''} onclose={() => (nav.namePrompt = null)}>
  <form class="space-y-3 px-2" onsubmit={submit}>
    <input
      class="h-11 w-full rounded-xl bg-surface px-4 text-[15px] outline-none"
      aria-label="Name"
      maxlength="100"
      autocomplete="off"
      bind:value={name}
    />
    {#if error}<p class="text-sm text-accent" role="alert">{error}</p>{/if}
    <div class="flex justify-end gap-2">
      <button type="button" class="rounded-full px-4 py-2 text-sm font-medium" onclick={() => (nav.namePrompt = null)}>Cancel</button>
      <button class="rounded-full bg-fg px-5 py-2 text-sm font-semibold text-bg disabled:opacity-50" disabled={busy || !name.trim()}>{prompt?.confirm ?? 'OK'}</button>
    </div>
  </form>
</Sheet>
