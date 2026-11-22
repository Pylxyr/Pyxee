<script lang="ts">
  import type { Snippet } from 'svelte';

  /** The top of a page: one title, a quiet line under it, the page's actions on the right, tabs or filters below. */
  let { title, meta = null, detail = null, kicker = null, lead, actions, children }: {
    title: string;
    /** A small line above the title (Home's greeting). */
    kicker?: string | null;
    meta?: string | null;
    /** More after the meta line, in the same line ("8 songs" then "· 31 min"). */
    detail?: string | null;
    /** Something before the title, such as a playlist's cover. */
    lead?: Snippet;
    actions?: Snippet;
    children?: Snippet;
  } = $props();
</script>

<div class="page-head">
  {@render lead?.()}
  <div class="page-head-main">
    {#if kicker}<p class="page-kicker">{kicker}</p>{/if}
    <h1 class="t-title truncate">{title}</h1>
    {#if meta}<p class="muted">{meta}{#if detail}<span>{" · "}{detail}</span>{/if}</p>{/if}
  </div>
  {#if actions}<div class="page-head-actions">{@render actions()}</div>{/if}
  {#if children}<div class="page-head-below">{@render children()}</div>{/if}
</div>
