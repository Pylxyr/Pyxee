<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLLiAttributes } from 'svelte/elements';
  import { availability } from '$lib/availability.svelte';
  import { artistOf, formatTime, titleOf } from '$lib/format';
  import { openTrackMenu } from '$lib/menu';
  import type { MenuExtra } from '$lib/nav.svelte';
  import { MOTION, listChangeMs, motionPx, reveal } from '$lib/motion';
  import { fly } from 'svelte/transition';
  import { cubicIn, expoOut } from 'svelte/easing';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import Thumb from './Thumb.svelte';

  type Props = Omit<HTMLLiAttributes, 'class' | 'children' | 'onplay'> & {
    track: TrackDto;
    /** Shows a place number before the cover. */
    index?: number;
    /** Replaces the artist line, in the accent colour ("Now playing", "Loading"). */
    status?: string;
    current?: boolean;
    /** Where a dragged row would land relative to this one. */
    over?: 'above' | 'below' | null;
    dragging?: boolean;
    onplay: () => void;
    handle?: Snippet;
    actions: Snippet;
    coverSize?: number;
    /** Lines this list adds to the song's menu ("Remove from queue"). */
    menuExtras?: MenuExtra[] | (() => MenuExtra[]);
    /** The row is an entry of the queue (the menu then offers to move it, not to add it again). */
    inQueue?: boolean;
  };

  // The row of the queue, of Up next in Now Playing and of a playlist: a handle, the song, and its own buttons.
  let { track, index, status = '', current = false, over = null, dragging = false, onplay, handle, actions, coverSize = 40, menuExtras = [], inQueue = false, ...rest }: Props = $props();

  // A song on this PC whose file is gone says so, before it is played and skipped.
  const missing = $derived(availability.isMissing(track.key));
  $effect(() => {
    void availability.epoch;
    availability.want(track.key);
  });

  // Right-click (or the menu key) on the row opens the song's menu, as the "more" button of a list row does.
  function openMenu(e: MouseEvent) {
    openTrackMenu(track, { event: e, opener: (e.currentTarget as HTMLElement).querySelector<HTMLElement>('.row-edit-main'), extras: typeof menuExtras === 'function' ? menuExtras() : menuExtras, queue: inQueue });
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<li class="row-edit" data-current={current} data-missing={missing} data-over={over} data-dragging={dragging} oncontextmenu={openMenu}
  use:reveal
  in:fly={{ y: motionPx(8), duration: listChangeMs(MOTION.base), easing: expoOut }}
  out:fly={{ x: motionPx(-24), duration: listChangeMs(MOTION.exit), easing: cubicIn }}
  {...rest}>
  {@render handle?.()}
  <button class="row-edit-main" onclick={onplay} title={missing ? 'This file is missing: moved, renamed, deleted, or its drive is not connected.' : undefined}>
    {#if index !== undefined}<span class="t-caption muted nums w-6 shrink-0 text-center" aria-hidden="true">{index + 1}</span>{/if}
    <Thumb src={track.thumbnail} seed={track.key} size={coverSize} rounded="r-sm" />
    <span class="min-w-0 flex-1">
      <span class="trow-title block" dir="auto" class:accent-text={current}>{titleOf(track)}</span>
      <span class="trow-artist block" dir="auto" style={status ? 'color:var(--accent-ink)' : undefined}>{status || (missing ? `File missing · ${artistOf(track)}` : artistOf(track))}</span>
    </span>
    <span class="trow-time">{formatTime(track.durationMs)}</span>
  </button>
  {@render actions()}
</li>
