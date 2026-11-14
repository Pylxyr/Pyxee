<script lang="ts">
  import type { GenreDto } from '$lib/bindings/GenreDto';
  import { GENRES_SHOWN } from '$lib/genres';

  /**
   * YouTube Music's genres as a row of chips, with "For you" (the usual page) first. Only the first few show; the rest
   * are behind "More genres", unless the one picked is among them.
   */
  let { genres, selected, onselect }: { genres: GenreDto[]; selected: GenreDto | null; onselect: (genre: GenreDto | null) => void } = $props();

  let expanded = $state(false);
  const hiddenPicked = $derived(selected !== null && genres.findIndex((g) => g.params === selected.params) >= GENRES_SHOWN);
  const shown = $derived(expanded || hiddenPicked ? genres : genres.slice(0, GENRES_SHOWN));
</script>

<div class="chips" role="group" aria-label="Genres">
  <button class="chip" aria-pressed={selected === null} onclick={() => onselect(null)}>For you</button>
  {#each shown as g (g.params)}
    <button class="chip" aria-pressed={selected?.params === g.params} onclick={() => onselect(g)}>{g.title}</button>
  {/each}
  {#if genres.length > GENRES_SHOWN && !(expanded || hiddenPicked)}
    <button class="chip chip-more" onclick={() => (expanded = true)}>More genres</button>
  {/if}
</div>
