<script lang="ts">
  import { player } from '$lib/player.svelte';
  import { lyricsStore } from '$lib/lyrics.svelte';
  import { currentLine } from '$lib/lyrics';

  let { active }: { active: boolean } = $props();

  const track = $derived(player.shown?.track ?? null);
  // Looked up when the tab is showing (and again when the song changes while it is).
  $effect(() => {
    if (active) void lyricsStore.load(track);
  });

  const st = $derived(lyricsStore.state);
  const lyrics = $derived(st.kind === 'ready' && st.key === track?.key ? st.lyrics : null);
  const loading = $derived(!lyrics && track != null && st.kind === 'loading');
  const index = $derived(lyrics?.kind === 'synced' ? currentLine(lyrics.lines, player.state.positionMs) : -1);

  // Follow the song, unless the user has just scrolled by hand.
  let box = $state<HTMLElement>();
  let handScrolledAt = 0;
  const byHand = () => (handScrolledAt = Date.now());
  $effect(() => {
    if (index < 0 || !box || Date.now() - handScrolledAt < 4000) return;
    box.querySelector('[data-current="true"]')?.scrollIntoView?.({ block: 'center', behavior: 'smooth' });
  });
</script>

<div
  bind:this={box}
  class="max-h-full overflow-y-auto px-3 py-4"
  data-testid="lyrics"
  onwheel={byHand}
  ontouchmove={byHand}
  onpointerdown={byHand}
  role="region"
  aria-label="Lyrics"
>
  {#if !track}
    <p class="muted py-6">Play a song to see its lyrics.</p>
  {:else if loading || !lyrics}
    <p class="muted py-6" role="status">Looking for lyrics…</p>
  {:else if lyrics.kind === 'synced'}
    <ol class="space-y-3 pb-24 pt-2">
      {#each lyrics.lines as line, i (i)}
        <li>
          <button
            class="lyric t-subtitle w-full text-left {i === index ? 'text-fg' : i < index ? 'text-muted/60' : 'text-muted'}"
            style="--d:{index < 0 ? 0 : Math.min(Math.abs(i - index), 4)}"
            data-current={i === index}
            onclick={() => player.seek(line.timeMs)}
          >{line.text || '♪'}</button>
        </li>
      {/each}
    </ol>
  {:else if lyrics.kind === 'plain'}
    <p class="t-lead whitespace-pre-wrap py-2 !font-normal">{lyrics.text}</p>
    <p class="t-caption muted mt-4">These lyrics are not timed, so they do not follow the song.</p>
  {:else if lyrics.kind === 'instrumental'}
    <p class="muted py-6">This one is instrumental, so there are no lyrics.</p>
  {:else if lyrics.kind === 'notFound'}
    <p class="muted py-6">No lyrics found for this song.</p>
  {:else}
    <p class="accent-text py-6" role="alert">
      Could not get the lyrics ({lyrics.message}).
      <button class="btn-link ml-2" onclick={() => lyricsStore.retry(track)}>Try again</button>
    </p>
  {/if}
</div>
