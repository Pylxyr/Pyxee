import type { TrackDto } from './bindings/TrackDto';
import { nav, type MenuExtra } from './nav.svelte';

/** A pop-up menu at the pointer needs room and a mouse; a narrow window or a touch screen gets the sheet. */
export function popupMenus(): boolean {
  return typeof matchMedia === 'function' && matchMedia('(min-width: 900px) and (pointer: fine)').matches;
}

interface Options {
  /** The right-click (or the Menu key) that asked for the menu: the pop-up opens at its position. */
  event?: MouseEvent;
  /** The \"more\" button that asked for it: the pop-up opens under it. */
  button?: HTMLElement | null;
  /** What gets focus back when the menu closes. */
  opener?: HTMLElement | null;
  extras?: MenuExtra[];
  /** The song is an entry of the queue: `extras` replace the lines that would add it to the queue again. */
  queue?: boolean;
}

/** Opens the song menu: a pop-up where there is room and a mouse, the sheet elsewhere. Every list uses this. */
export function openTrackMenu(track: TrackDto, { event, button, opener = null, extras = [], queue = false }: Options = {}) {
  event?.preventDefault();
  nav.menuOpener = opener ?? button ?? null;
  nav.menuExtras = extras;
  nav.menuQueue = queue;
  if (popupMenus() && event) nav.menuAt = { x: event.clientX, y: event.clientY, align: 'start' };
  else if (popupMenus() && button) {
    const r = button.getBoundingClientRect();
    nav.menuAt = { x: r.right, y: r.bottom + 4, align: 'end' };
  } else nav.menuAt = null;
  nav.menuTrack = track;
}
