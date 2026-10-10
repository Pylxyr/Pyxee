<script lang="ts">
  import { formatTime } from '$lib/format';

  let { positionMs, durationMs, onseek, label = 'Seek', accent = false, disabled = false }: {
    positionMs: number;
    durationMs: number | null;
    onseek: (ms: number) => void;
    label?: string;
    accent?: boolean;
    disabled?: boolean;
  } = $props();

  // While dragging, show the thumb position instead of the (older) backend position.
  let dragging = $state<number | null>(null);
  const max = $derived(durationMs ?? 0);
  const value = $derived(Math.min(dragging ?? positionMs, max || positionMs));
  const pct = $derived(max > 0 ? Math.min(100, (value / max) * 100) : 0);
</script>

<input
  type="range"
  class="slider {accent ? 'accent' : ''}"
  aria-label={label}
  aria-valuetext="{formatTime(value)} of {formatTime(durationMs)}"
  min="0"
  {max}
  step="250"
  {value}
  disabled={disabled || !durationMs}
  style="--pct:{pct}%"
  oninput={(e) => (dragging = e.currentTarget.valueAsNumber)}
  onchange={(e) => {
    onseek(e.currentTarget.valueAsNumber);
    dragging = null;
  }}
/>
