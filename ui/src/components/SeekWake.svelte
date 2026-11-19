<script lang="ts">
  // A new song lights the seek line: a streak of the accent colour runs along it once, left to right, as the new cover
  // dissolves in. It is drawn once and only moved (transform and opacity, 700 ms, then gone: see `.seek-wake` in
  // app.css), so it costs nothing while a song plays. Keyed by the song, so each new song replays it; the first one
  // after the window opens too.
  let { songKey }: { songKey: string } = $props();
</script>

{#key songKey}
  <span class="seek-wake" aria-hidden="true"><i></i></span>
{/key}
