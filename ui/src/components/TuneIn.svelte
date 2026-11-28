<script lang="ts">
  import { library } from '$lib/library.svelte';
  import { player } from '$lib/player.svelte';
  import { playWithRelated } from '$lib/radio';
  import { Loaded } from '$lib/loaded.svelte';
  import { tuneInSeeds } from '$lib/tunein';
  import type { HomeDto } from '$lib/bindings/HomeDto';
  import MixCard from './MixCard.svelte';
  import SectionHead from './SectionHead.svelte';

  /**
   * "Tune in" on Explore: one card per artist you come back to, each the start of a radio (a song of theirs, then songs like
   * it: new music). It is Explore's because it is for finding music you have not heard; Home is for what you have. The
   * cards come from your favourites and history and follow them as the library changes. A late answer never replaces a
   * newer one, and with nothing to offer (a new library) the row is simply not there.
   */
  const source = new Loaded<HomeDto>();
  $effect(() => {
    void library.revision;
    const b = library.backend;
    if (b) void source.load(() => b.homeGet());
  });
  const seeds = $derived(tuneInSeeds(source.data?.quickPicks ?? [], source.data?.recent ?? [], player.shown?.track.key ?? null));
</script>

{#if seeds.length > 0}
  <section class="min-w-0">
    <SectionHead>Tune in</SectionHead>
    <ul class="mix-grid" aria-label="Start a radio">
      {#each seeds as t (t.key)}
        <li><MixCard track={t} onpick={() => void playWithRelated(t)} /></li>
      {/each}
    </ul>
  </section>
{/if}
