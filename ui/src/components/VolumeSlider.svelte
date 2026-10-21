<script lang="ts">
  import { prefs } from '$lib/prefs.svelte';
  import Icon from './Icon.svelte';
</script>

<div class="flex items-center gap-2 text-muted">
  <Icon name="volume" size={20} />
  <input
    type="range"
    class="slider"
    min="0"
    max="100"
    value={prefs.volume}
    aria-label="Volume"
    style="--pct:{prefs.volume}%"
    oninput={(e) => prefs.setVolume(e.currentTarget.valueAsNumber)}
  />
</div>
