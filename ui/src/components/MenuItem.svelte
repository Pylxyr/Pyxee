<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  /** One row of a sheet's list: a menu entry, a sleep-timer length, a playlist to add to. */
  type Props = Omit<HTMLButtonAttributes, 'class' | 'children'> & { class?: string; children: Snippet };
  let { class: extra = '', children, ...rest }: Props = $props();
</script>

<button class="menu-item {extra}" {...rest}>{@render children()}</button>
