<script lang="ts">
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import { artistOf, hueFor, titleOf } from '$lib/format';
  import Icon from './Icon.svelte';
  import Thumb from './Thumb.svelte';

  /** One way to start a radio from Home: a song's colours and cover ("Tune in"), or the "Surprise me" card (no song). */
  let { track = null, onpick }: { track?: TrackDto | null; onpick: () => void } = $props();

  // Songs whose keys are close (a playlist's ids) still get colours well apart: a step of 137 degrees around the wheel.
  const hueOf = (t: TrackDto) => (hueFor(t.key) * 137) % 360;
</script>

{#if track}
  <button class="mix-card" style="--mix-h: {hueOf(track)}" aria-label="Start a radio from {titleOf(track)}" onclick={onpick}>
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
