<script lang="ts">
  /** Three bars that move while the song plays and stand still while it is paused. Decorative: the row says the rest. */
  let { playing = false }: { playing?: boolean } = $props();
</script>

<span class="eq" data-playing={playing} aria-hidden="true"><i></i><i></i><i></i></span>
