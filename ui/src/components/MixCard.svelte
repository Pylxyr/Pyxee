<script lang="ts">
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import { cardTones } from '$lib/color';
  import { coverColorsOf } from '$lib/covercolors.svelte';
  import { artistOf, hueFor, titleOf } from '$lib/format';
  import { look } from '$lib/look.svelte';
  import Icon from './Icon.svelte';
  import Thumb from './Thumb.svelte';

  /** One way to start a radio from Home: a song's colours and cover ("Tune in"), or the "Surprise me" card (no song). */
  let { track = null, onpick }: { track?: TrackDto | null; onpick: () => void } = $props();

  // Songs whose keys are close (a playlist's ids) still get colours well apart: a step of 137 degrees around the wheel.
  const hueOf = (t: TrackDto) => (hueFor(t.key) * 137) % 360;

  // The card wears the cover's own colours. Until they are read it has the song's stand-in colour (the one its cover
  // shows until the picture is there), and the two blend over a moment. With the theme's colour chosen nothing is read.
  const colors = coverColorsOf(() => (look.value.tune === 'cover' ? (track?.thumbnail ?? null) : null));
  const tones = $derived(colors.value ? cardTones(colors.value) : null);
  const style = $derived(track ? `--mix-h: ${hueOf(track)}${tones ? `; --mix-a: ${tones.a}; --mix-b: ${tones.b}` : ''}` : undefined);
</script>

{#if track}
  <button class="mix-card" {style} aria-label="Start a radio from {titleOf(track)}" onclick={onpick}>
    <span class="mix-art"><Thumb src={track.thumbnail} seed={track.key} size={112} fill rounded="r-sm" /></span>
    <span class="mix-kicker">Radio</span>
    <span class="mix-title" dir="auto">{titleOf(track)}</span>
    <span class="mix-sub" dir="auto">Songs like this · {artistOf(track)}</span>
    <span class="mix-play" aria-hidden="true"><Icon name="play" size={18} fill /></span>
  </button>
{:else}
  <button class="mix-card mix-surprise" aria-label="Surprise me" onclick={onpick}>
    <span class="mix-dice" aria-hidden="true"><Icon name="shuffle" size={46} /></span>
    <span class="mix-kicker">Feeling lucky</span>
    <span class="mix-title">Surprise me</span>
    <span class="mix-sub">A random radio from your songs</span>
    <span class="mix-play" aria-hidden="true"><Icon name="play" size={18} fill /></span>
  </button>
{/if}
