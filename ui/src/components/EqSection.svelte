<script lang="ts">
  import { eq } from '$lib/eq.svelte';
  import { BAND_HZ, EQ_LIMIT, EQ_PRESETS, EQ_STEP, FLAT_EQ, bandLabel, bandName, formatDb, formatGain, presetOf } from '$lib/eq';
  import Button from './Button.svelte';
  import Segmented from './Segmented.svelte';
  import Toggle from './Toggle.svelte';

  const preset = $derived(presetOf(eq.settings.gains));
  const flat = $derived(preset?.id === 'flat' && eq.settings.preamp === 0);

  /** Where the slider fills from and to: from the middle (0 dB) to the thumb. */
  function fill(tenths: number): string {
    const at = ((tenths + EQ_LIMIT) / (2 * EQ_LIMIT)) * 100;
    return `--lo:${Math.min(50, at)}%;--hi:${Math.max(50, at)}%`;
  }
</script>

<section class="settings-section eq-section" aria-label="Equalizer">
  <Toggle
    label="Equalizer"
    hint="Ten bands from 31 Hz to 16 kHz. A change is heard at once. When bands are raised the whole sound is turned down a little, on purpose, so that a boost cannot distort."
    checked={eq.settings.enabled}
    onchange={(on) => eq.update({ enabled: on }, true)}
  />
  <div class="look-group">
    <p class="look-label">Preset</p>
    <Segmented
      mode="toggle"
      label="Preset"
      items={EQ_PRESETS.map((p) => ({ id: p.id, label: p.label }))}
      value={preset?.id ?? ''}
      onselect={(id) => eq.update({ gains: [...EQ_PRESETS.find((p) => p.id === id)!.gains] }, true)}
    />
  </div>
  <div class="eq-bands" class:eq-off={!eq.settings.enabled} role="group" aria-label="Bands">
    {#each BAND_HZ as hz, i (hz)}
      {@const gain = eq.settings.gains[i] ?? 0}
      <div class="eq-col">
        <span class="eq-db" aria-hidden="true">{formatGain(gain)}</span>
        <span class="eq-track">
          <input
            type="range"
            class="eq-band"
            min={-EQ_LIMIT}
            max={EQ_LIMIT}
            step={EQ_STEP}
            value={gain}
            style={fill(gain)}
            aria-label={bandName(hz)}
            aria-valuetext={formatDb(gain)}
            oninput={(e) => eq.setGain(i, e.currentTarget.valueAsNumber, false)}
            onchange={(e) => eq.setGain(i, e.currentTarget.valueAsNumber, true)}
          />
        </span>
        <span class="eq-hz" aria-hidden="true">{bandLabel(hz)}</span>
      </div>
    {/each}
  </div>
  <div class="look-group" class:eq-off={!eq.settings.enabled}>
    <label class="eq-preamp">
      <span class="look-label">Preamp</span>
      <input
        type="range"
        class="slider accent"
        min={-EQ_LIMIT}
        max={EQ_LIMIT}
        step={EQ_STEP}
        value={eq.settings.preamp}
        style="--pct:{((eq.settings.preamp + EQ_LIMIT) / (2 * EQ_LIMIT)) * 100}%"
        aria-label="Preamp"
        aria-valuetext={formatDb(eq.settings.preamp)}
        oninput={(e) => eq.update({ preamp: e.currentTarget.valueAsNumber }, false)}
        onchange={(e) => eq.update({ preamp: e.currentTarget.valueAsNumber }, true)}
      />
      <span class="eq-db" aria-hidden="true">{formatDb(eq.settings.preamp)}</span>
    </label>
  </div>
  {#if !flat}
    <div class="mt-2 px-3"><Button onclick={() => eq.update({ gains: [...FLAT_EQ.gains], preamp: 0 }, true)}>Reset to flat</Button></div>
  {/if}
  {#if eq.error}<p class="accent-text px-3" role="alert">Could not change the equalizer ({eq.error}).</p>{/if}
</section>
