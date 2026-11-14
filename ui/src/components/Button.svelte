<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  type Props = Omit<HTMLButtonAttributes, 'class' | 'children'> & {
    /** `primary` carries the accent (one per view); `quiet` has no frame until it is pointed at. */
    variant?: 'primary' | 'secondary' | 'quiet';
    size?: 'sm' | 'md';
    class?: string;
    children: Snippet;
  };

  let { variant = 'secondary', size = 'md', class: extra = '', children, ...rest }: Props = $props();
</script>

<button class="btn {variant === 'secondary' ? '' : `btn-${variant}`} {size === 'sm' ? 'btn-sm' : ''} {extra}" {...rest}>{@render children()}</button>
