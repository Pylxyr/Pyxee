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
    <p class="py-6 text-sm text-muted">Play a song to see its lyrics.</p>
  {:else if loading || !lyrics}
    <p class="py-6 text-sm text-muted" role="status">Looking for lyrics…</p>
  {:else if lyrics.kind === 'synced'}
    <ol class="space-y-3 pb-24 pt-2">
      {#each lyrics.lines as line, i (i)}
        <li>
          <button
            class="w-full text-left text-xl font-semibold leading-snug transition-colors {i === index ? 'text-fg' : i < index ? 'text-muted/60' : 'text-muted'}"
            data-current={i === index}
            onclick={() => player.seek(line.timeMs)}
          >{line.text || '♪'}</button>
        </li>
      {/each}
    </ol>
  {:else if lyrics.kind === 'plain'}
    <p class="whitespace-pre-wrap py-2 text-base leading-relaxed">{lyrics.text}</p>
    <p class="mt-4 text-[13px] text-muted">These lyrics are not timed, so they do not follow the song.</p>
  {:else if lyrics.kind === 'instrumental'}
    <p class="py-6 text-sm text-muted">This one is instrumental, so there are no lyrics.</p>
  {:else if lyrics.kind === 'notFound'}
    <p class="py-6 text-sm text-muted">No lyrics found for this song.</p>
  {:else}
    <p class="py-6 text-sm text-accent" role="alert">
      Could not get the lyrics ({lyrics.message}).
      <button class="ml-2 font-semibold underline" onclick={() => lyricsStore.retry(track)}>Try again</button>
    </p>
  {/if}
</div>
