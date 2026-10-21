<script lang="ts">
  // A settings row with a switch. The switch is a real checkbox, so its role, Space key and screen-reader name are
  // the browser's own. The hint is read as its description.
  let { label, hint = '', checked, onchange }: { label: string; hint?: string; checked: boolean; onchange: (checked: boolean) => void } = $props();
  const uid = $props.id();
  const hintId = `toggle-hint-${uid}`;
</script>

<label class="setting">
  <span class="min-w-0 flex-1">{label}</span>
  <input type="checkbox" class="switch" {checked} aria-describedby={hint ? hintId : undefined} onchange={(e) => onchange(e.currentTarget.checked)} />
</label>
{#if hint}<p id={hintId} class="setting-hint px-3 pb-2">{hint}</p>{/if}
