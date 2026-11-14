<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import Icon from './Icon.svelte';

  type Props = Omit<HTMLButtonAttributes, 'class'> & {
    icon: string;
    /** The accessible name: an icon alone says nothing to a screen reader. */
    label: string;
    size?: 'md' | 'lg';
    iconSize?: number;
    fill?: boolean;
    /** Set for a button that switches something on and off (it then reads as pressed and turns accent). */
    pressed?: boolean;
    /** Draw the icon in the text colour instead of the quiet one. */
    strong?: boolean;
    class?: string;
  };

  let { icon, label, size = 'md', iconSize = 20, fill = false, pressed, strong = false, class: extra = '', children, ...rest }: Props = $props();
</script>

<button class="icon-btn {size === 'lg' ? 'icon-btn-lg' : ''} {strong ? 'icon-btn-strong' : ''} {extra}" aria-label={label} aria-pressed={pressed} {...rest}>
  <Icon name={icon} size={iconSize} {fill} />
  {@render children?.()}
</button>
