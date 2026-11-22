<script lang="ts">
  import { look } from '$lib/look.svelte';
  import { seekMode, setSeekMode } from '$lib/seekmode.svelte';
  import { CORNERS, COVERS, HEROES, PANELS, SEEK_CHOICES, SURFACES, TUNES, DEFAULT_LOOK, type Look } from '$lib/look';
  import type { SeekMode } from '$lib/seekbar';
  import Button from './Button.svelte';
  import Segmented from './Segmented.svelte';

  type Group = { key: keyof Look; label: string; items: readonly { id: string; label: string; hint: string }[] };
  const GROUPS: Group[] = [
    { key: 'corners', label: 'Corners', items: CORNERS },
    { key: 'surfaces', label: 'Surfaces', items: SURFACES },
    { key: 'panels', label: 'Panels', items: PANELS },
    { key: 'covers', label: 'Cover shape', items: COVERS },
    { key: 'hero', label: 'Home top card and Now Playing', items: HEROES },
    { key: 'tune', label: 'Home: Tune in cards', items: TUNES },
  ];
  const hintOf = (items: Group['items'], id: string) => items.find((i) => i.id === id)?.hint ?? '';
  const changed = $derived((Object.keys(DEFAULT_LOOK) as (keyof Look)[]).some((k) => look.value[k] !== DEFAULT_LOOK[k]));
</script>

<section class="settings-section look-section" aria-label="Look">
  {#each GROUPS as g (g.key)}
    <div class="look-group">
      <p class="look-label" id="look-{g.key}">{g.label}</p>
      <Segmented
        mode="toggle"
        label={g.label}
        items={g.items.map((i) => ({ id: i.id, label: i.label }))}
        value={look.value[g.key]}
        onselect={(id) => look.set(g.key, id as never)}
      />
      <p class="setting-hint">{hintOf(g.items, look.value[g.key])}</p>
      {#if g.key === 'hero' && look.value.surfaces === 'flat'}
        <p class="setting-hint">Now Playing stays plain while Surfaces is Flat.</p>
      {/if}
    </div>
  {/each}
  <div class="look-group">
    <p class="look-label" id="look-seek">Progress bar</p>
    <Segmented
      mode="toggle"
      label="Progress bar"
      items={SEEK_CHOICES.map((i) => ({ id: i.id, label: i.label }))}
      value={seekMode.value === 'pixel' ? 'step' : seekMode.value}
      onselect={(id) => setSeekMode(id as SeekMode)}
    />
    <p class="setting-hint">{hintOf(SEEK_CHOICES, seekMode.value === 'pixel' ? 'step' : seekMode.value)}</p>
  </div>
  {#if changed}
    <div class="mt-2"><Button onclick={() => look.reset()}>Reset the look</Button></div>
  {/if}
</section>
