<script lang="ts">
  import { tick } from 'svelte';
  import type { TrackDto } from '$lib/bindings/TrackDto';
  import { nav } from '$lib/nav.svelte';
  import { player } from '$lib/player.svelte';
  import { library } from '$lib/library.svelte';
  import { search } from '$lib/search.svelte';
  import { snackbar } from '$lib/snackbar.svelte';
  import { playWithRelated } from '$lib/radio';
  import { artistOf, titleOf } from '$lib/format';
  import { tossToQueue } from '$lib/toss';
  import Icon from './Icon.svelte';
  import MenuItem from './MenuItem.svelte';
  import Sheet from './Sheet.svelte';
  import Thumb from './Thumb.svelte';

  // The menu a row's right-click or \"More options\" button opens. A wide window with a mouse gets a pop-up beside the
  // pointer; anything else (a narrow window, touch) gets the in-page sheet. Never a native menu.
  // The notices name the song (\"Saturn will play next\"); a very long title is cut short.
  const short = (t: TrackDto) => (t.title.length > 48 ? `${t.title.slice(0, 47).trimEnd()}…` : t.title);
  function done(label: string, run: () => void, toss = false) {
    // The song is thrown at the queue before the menu closes, from the row the menu was opened on.
    if (toss) tossToQueue(nav.menuOpener);
    run();
    nav.closeMenu();
    snackbar.push(label);
  }
  /** Closes the menu first, then does something that opens another sheet or page. The track is
   *  captured before closing: the menu's `{@const}` goes null the moment `menuTrack` is cleared. */
  function leave(run: (track: TrackDto) => void) {
    const track = nav.menuTrack;
    if (!track) return;
    nav.closeMenu();
    run(track);
  }
  function extra(run: () => void) {
    nav.closeMenu();
    run();
  }

  let menu = $state<HTMLElement>();
  const popup = $derived(nav.menuAt !== null && nav.menuTrack !== null);

  /** Puts the pop-up where it was asked for, kept inside the window, before it is first drawn. */
  function place(node: HTMLElement, at: { x: number; y: number; align: 'start' | 'end' }) {
    const gap = 8;
    const w = node.offsetWidth;
    const h = node.offsetHeight;
    let left = at.align === 'end' ? at.x - w : at.x;
    let top = at.y;
    if (left + w > innerWidth - gap) left = innerWidth - gap - w;
    if (top + h > innerHeight - gap) top = at.align === 'end' ? Math.max(gap, at.y - h - 40) : Math.max(gap, innerHeight - gap - h);
    node.style.left = `${Math.max(gap, left)}px`;
    node.style.top = `${Math.max(gap, top)}px`;
    node.dataset.placed = '';
  }

  // Focus goes into the pop-up and back to what opened it, as a sheet does.
  $effect(() => {
    if (!popup) return;
    const restore = nav.menuOpener?.isConnected ? nav.menuOpener : null;
    void tick().then(() => menu?.querySelector<HTMLElement>('[role=\"menuitem\"]')?.focus({ preventScroll: true }));
    return () => restore?.focus({ preventScroll: true });
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      nav.closeMenu();
      return;
    }
    if (e.key === 'Tab') {
      e.preventDefault();
      nav.closeMenu();
      return;
    }
    const items = [...(menu?.querySelectorAll<HTMLElement>('[role=\"menuitem\"]:not(:disabled)') ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const to = e.key === 'ArrowDown' ? at + 1 : e.key === 'ArrowUp' ? at - 1 : e.key === 'Home' ? 0 : e.key === 'End' ? items.length - 1 : null;
    if (to === null || items.length === 0) return;
    e.preventDefault();
    items[(to + items.length) % items.length]?.focus();
  }

  /** A right-click away from the menu closes it and goes on to whatever was under the pointer (another row's menu). */
  async function scrimContext(e: MouseEvent) {
    e.preventDefault();
    const { clientX, clientY } = e;
    nav.closeMenu();
    await tick();
    document.elementFromPoint(clientX, clientY)?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX, clientY }));
  }
</script>

{#snippet extraLines(role: 'menuitem' | undefined, danger: boolean)}
  {#each nav.menuExtras.filter((x) => !!x.danger === danger) as x (x.label)}
    <button class="ctx-item" data-danger={x.danger ? '' : undefined} {role} onclick={() => extra(x.run)}>{#if x.icon}<Icon name={x.icon} size={16} />{:else}<span class="ctx-gap"></span>{/if}<span>{x.label}</span></button>
  {/each}
{/snippet}

{#snippet lines(t: TrackDto, role: 'menuitem' | undefined)}
  {#if nav.menuQueue}
    {@render extraLines(role, false)}
  {:else}
    <button class="ctx-item" {role} onclick={() => done(`${short(t)} is playing now`, () => player.playTracks([t]))}><Icon name="play" size={16} fill /><span>Play now</span></button>
    <button class="ctx-item" {role} onclick={() => done(`${short(t)} will play next`, () => player.playNext(t), true)}><Icon name="top" size={16} /><span>Play next</span></button>
    <button class="ctx-item" {role} onclick={() => done(`${short(t)} is added to queue`, () => player.addToQueue(t), true)}><Icon name="list-plus" size={16} /><span>Add to queue</span></button>
  {/if}
  <div class="ctx-sep" role="separator"></div>
  <button class="ctx-item" {role} onclick={() => done(`Starting a radio from ${short(t)}`, () => void playWithRelated(t))}><Icon name="radio" size={16} /><span>Start radio</span></button>
  <button class="ctx-item" {role} onclick={() => leave((track) => library.toggleLike(track))}><Icon name="heart" size={16} fill={library.isLiked(t.key)} /><span>{library.isLiked(t.key) ? 'Unlike' : 'Like'}</span></button>
  <button class="ctx-item" {role} onclick={() => leave((track) => (nav.pickTrack = track))}><Icon name="plus" size={16} /><span>Add to playlist</span></button>
  <button class="ctx-item" {role} onclick={() => leave((track) => { nav.go('search'); search.run(track.artist, 'songs'); })}><Icon name="user" size={16} /><span>Go to artist</span></button>
  {#if nav.menuQueue}
    {#if nav.menuExtras.some((x) => x.danger)}<div class="ctx-sep" role="separator"></div>{/if}
    {@render extraLines(role, true)}
  {:else if nav.menuExtras.length > 0}
    <div class="ctx-sep" role="separator"></div>
    {@render extraLines(role, false)}
    {@render extraLines(role, true)}
  {/if}
{/snippet}

{#if popup && nav.menuTrack && nav.menuAt}
  {@const t = nav.menuTrack}
  <div class="ctx-scrim" role="presentation" data-testid="ctx-scrim" onpointerdown={() => nav.closeMenu()} oncontextmenu={scrimContext}></div>
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div bind:this={menu} class="ctx popover bg-solid-2" role="menu" aria-label={t.title} data-testid="ctx-menu" {onkeydown} use:place={nav.menuAt}>
    <div class="ctx-head" role="presentation">
      <Thumb src={t.thumbnail} seed={t.key} size={36} rounded="r-sm" />
      <span class="min-w-0 flex-1">
        <span class="trow-title block" dir="auto">{titleOf(t)}</span>
        <span class="trow-artist block" dir="auto">{artistOf(t)}</span>
      </span>
    </div>
    {@render lines(t, 'menuitem')}
  </div>
{:else}
  <Sheet open={nav.menuTrack !== null} title={nav.menuTrack?.title ?? ''} returnFocus={nav.menuOpener} onclose={() => nav.closeMenu()}>
    {#if nav.menuTrack}
      {@const t = nav.menuTrack}
      {#if nav.menuQueue}
        {#each nav.menuExtras.filter((x) => !x.danger) as x (x.label)}
          <MenuItem onclick={() => extra(x.run)}>{x.label}</MenuItem>
        {/each}
      {:else}
        <MenuItem onclick={() => done(`${short(t)} is playing now`, () => player.playTracks([t]))}>Play now</MenuItem>
        <MenuItem onclick={() => done(`${short(t)} will play next`, () => player.playNext(t), true)}>Play next</MenuItem>
        <MenuItem onclick={() => done(`${short(t)} is added to queue`, () => player.addToQueue(t), true)}>Add to queue</MenuItem>
      {/if}
      <MenuItem onclick={() => leave((track) => library.toggleLike(track))}>{library.isLiked(t.key) ? 'Unlike' : 'Like'}</MenuItem>
      <MenuItem onclick={() => leave((track) => (nav.pickTrack = track))}>Add to playlist</MenuItem>
      <MenuItem onclick={() => leave((track) => { nav.go('search'); search.run(track.artist, 'songs'); })}>Go to artist</MenuItem>
      {#each nav.menuExtras.filter((x) => (nav.menuQueue ? x.danger : true)) as x (x.label)}
        <MenuItem onclick={() => extra(x.run)}>{x.label}</MenuItem>
      {/each}
    {/if}
  </Sheet>
{/if}
