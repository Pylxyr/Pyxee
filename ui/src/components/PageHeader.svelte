<script lang="ts">
  import type { Snippet } from 'svelte';

  /** The top of a page: one title, a quiet line under it, the page's actions on the right, tabs or filters below. */
  let { title, meta = null, detail = null, kicker = null, inline = false, quiet = false, lead, actions, children }: {
    title: string;
    /** A small line above the title (Home's greeting). */
    kicker?: string | null;
    meta?: string | null;
    /** The tabs or filters in `children` are short (a handful of words): from 900 px they sit on the title's line, and wrap
     *  below it as one block when there is no room. Without it (a row of genre chips, which can need two lines of their own)
     *  they always have their own full-width row under the title. */
    inline?: boolean;
    /** A small, quiet title (Home's greeting): the page leads with something else. */
    quiet?: boolean;
    /** More after the meta line, in the same line ("8 songs" then "· 31 min"). */
    detail?: string | null;
    /** Something before the title, such as a playlist's cover. */
    lead?: Snippet;
    actions?: Snippet;
    children?: Snippet;
  } = $props();
</script>

<div class="page-head" data-inline={inline && children ? '' : undefined} data-quiet={quiet ? '' : undefined}>
  {@render lead?.()}
  <div class="page-head-main">
    {#if kicker}<p class="page-kicker">{kicker}</p>{/if}
    <h1 class="t-title truncate">{title}</h1>
    {#if meta}<p class="muted">{meta}{#if detail}<span>{" · "}{detail}</span>{/if}</p>{/if}
  </div>
  {#if actions}<div class="page-head-actions">{@render actions()}</div>{/if}
  {#if children}<div class="page-head-below">{@render children()}</div>{/if}
</div>
