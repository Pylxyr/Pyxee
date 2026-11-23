<script lang="ts">
  import { fade } from 'svelte/transition';
  import { motionMs } from '$lib/motion';

  /**
   * One cover-colored background, drawn as an opaque layer behind its parent's content (the CSS of each use says how).
   * When `id` changes (the colors of a new cover arrived) the new layer fades in over the old one and the old one is taken
   * away once it is fully covered. That is opacity only, so the compositor does it: nothing is repainted per frame, which
   * a transition of the colors themselves (custom properties) would do on the main thread for as long as it lasts. The
   * first layer of a parent is simply there; under reduced motion a new one replaces the old at once.
   */
  let { id, look, class: cls = '', style }: { id: string; look: string; class?: string; style?: string } = $props();
</script>

{#key id}
  <span class="wash {cls}" data-wash={look} {style} aria-hidden="true" out:fade={{ delay: motionMs(700), duration: 0 }}></span>
{/key}
