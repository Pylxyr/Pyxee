<script lang="ts">
  import { formatTime } from '$lib/format';
  import { player, STALE_AFTER_MS } from '$lib/player.svelte';
  import { seekMode } from '$lib/seekmode.svelte';
  import { DRIFT_RESYNC_MS, animatedPositionMs, devicePixels, nextPixelDelayMs, pixelPct, stepsFor } from '$lib/seekbar';

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
  const exactPct = $derived(max > 0 ? Math.min(100, (value / max) * 100) : 0);

  // How the fill moves (PYXEE_SEEK_MODE): `step` = once a second, with the reports; `pixel` = one timer per whole pixel;
  // `glide` / `steps` = one transform animation for the rest of the song. The time labels and the slider's own value stay at 1 Hz.
  const mode = $derived(durationMs ? seekMode.value : 'step');
  const glide = $derived(mode === 'glide' || mode === 'steps');

  // Only while the page is visible: a hidden window arms nothing.
  let visible = $state(typeof document === 'undefined' || !document.hidden);
  $effect(() => {
    const on = () => (visible = !document.hidden);
    document.addEventListener('visibilitychange', on);
    return () => document.removeEventListener('visibilitychange', on);
  });

  // The bar's real width in device pixels, watched only while a smooth mode is on and the bar is in the page.
  let slider = $state<HTMLInputElement>();
  let px = $state(0);
  $effect(() => {
    if (mode === 'step' || !slider || typeof ResizeObserver === 'undefined') return;
    const el = slider;
    const read = () => (px = devicePixels(el.getBoundingClientRect().width, window.devicePixelRatio));
    read();
    const ro = new ResizeObserver(read);
    ro.observe(el);
    return () => ro.disconnect();
  });

  // `pixel`: where the fill is, in ms; moved by one timer set for the next pixel boundary (like the lyric lines' timer in
  // LyricsPanel: from the last report plus the time since, nothing armed while paused, hidden or when reports stopped).
  let shownMs = $state(0);
  let due = $state(0);
  $effect(() => {
    due;
    if (mode !== 'pixel') return;
    void player.positionMs; // a new report re-aligns the clock
    const now = player.estimatedPositionMs();
    shownMs = now;
    if (!visible || px <= 0 || !player.isPlaying || player.positionAgeMs() >= STALE_AFTER_MS) return;
    const delay = nextPixelDelayMs(now, max, px);
    if (delay == null) return;
    const timer = setTimeout(() => due++, delay);
    return () => clearTimeout(timer);
  });

  // `glide` / `steps`: the fill is a rectangle scaled from the left by one WAAPI animation (compositor thread), started
  // from the last report and ending with the song. It is only restarted on a seek, a new song, a drift over 300 ms, or
  // after a pause; one timer stops it when reports stop arriving.
  let fill = $state<HTMLElement>();
  let anim: Animation | null = null;
  let animStart = 0; // the song position the animation started from
  let animFor = ''; // what it was started for: length + mode
  let stopTimer: ReturnType<typeof setTimeout> | undefined;
  const stopAnim = () => {
    clearTimeout(stopTimer);
    anim?.cancel();
    anim = null;
  };
  $effect(() => {
    if (!glide || !fill) return;
    void player.positionMs;
    const drag = dragging;
    const playing = player.isPlaying;
    const now = player.estimatedPositionMs();
    const at = (ms: number) => (max > 0 ? Math.min(1, Math.max(0, ms / max)) : 0);
    if (drag != null || !playing || !visible || max <= 0 || typeof fill.animate !== 'function') {
      stopAnim();
      fill.style.transform = `scaleX(${at(drag ?? now)})`;
      return;
    }
    const key = `${max}:${mode}:${px}`;
    const running = anim && anim.playState === 'running' && animFor === key;
    const drift = running ? Math.abs(animatedPositionMs(animStart, Number(anim!.currentTime ?? 0), max) - now) : Infinity;
    if (drift > DRIFT_RESYNC_MS) {
      stopAnim();
      const remaining = max - now;
      if (remaining > 0) {
        const easing = mode === 'steps' ? `steps(${stepsFor(remaining, Math.ceil(px * (1 - at(now))))}, jump-end)` : 'linear';
        anim = fill.animate([{ transform: `scaleX(${at(now)})` }, { transform: 'scaleX(1)' }], { duration: remaining, easing, fill: 'forwards' });
        animStart = now;
        animFor = key;
      }
    }
    // Reports stopped (the window was asleep): freeze the bar where it is instead of letting it run on an old clock.
    clearTimeout(stopTimer);
    stopTimer = setTimeout(() => anim?.pause(), Math.max(0, STALE_AFTER_MS - player.positionAgeMs()));
  });
  $effect(() => stopAnim);

  const pct = $derived(glide ? 0 : mode === 'pixel' && dragging == null && px > 0 ? pixelPct(shownMs, max, px) : exactPct);
</script>

{#if !durationMs}
  <!-- Length unknown: no fake scrubber, just an honest "playing, length not known" bar. -->
  <div
    class="seek-unknown {accent ? 'accent' : ''}"
    role="progressbar"
    aria-label={label}
    aria-valuetext="{formatTime(positionMs)} elapsed, length unknown"
    title="Length unknown"
  ></div>
{:else}
{#snippet range()}
<input
  bind:this={slider}
  type="range"
  class="slider {accent ? 'accent' : ''} {glide ? 'glide' : ''}"
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
{/snippet}
{#if glide}
  <div class="seek-wrap">
    <div class="seek-clip"><div class="seek-fill {accent ? 'accent' : ''}" bind:this={fill}></div></div>
    {@render range()}
  </div>
{:else}
  {@render range()}
{/if}
{/if}
