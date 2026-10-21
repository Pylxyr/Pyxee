<script lang="ts">
  import { nav } from '$lib/nav.svelte';
  import { messageOf } from '$lib/backend';
  import Button from './Button.svelte';
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
  <form class="space-y-3 px-1" onsubmit={submit}>
    <input class="field" aria-label="Name" maxlength="100" autocomplete="off" bind:value={name} />
    {#if error}<p class="accent-text" role="alert">{error}</p>{/if}
    <div class="flex justify-end gap-2">
      <Button type="button" variant="quiet" onclick={() => (nav.namePrompt = null)}>Cancel</Button>
      <Button type="submit" variant="primary" disabled={busy || !name.trim()}>{prompt?.confirm ?? 'OK'}</Button>
    </div>
  </form>
</Sheet>
