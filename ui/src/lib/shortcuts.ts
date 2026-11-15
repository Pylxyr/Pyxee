import type { CommandDto } from './bindings/CommandDto';

export type KeyAction =
  | { kind: 'command'; command: CommandDto }
  | { kind: 'close' }
  | { kind: 'fullscreen' }
  | { kind: 'search' }
  | null;

export interface KeyContext {
  positionMs: number;
  durationMs: number | null;
  nowPlayingOpen: boolean;
  hasTrack: boolean;
  /** The focus got where it is by a pointer click, not by the keyboard (see `App.svelte`). Optional: false when unknown. */
  pointerFocus?: boolean;
}

interface KeyLike {
  key: string;
  ctrlKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
  target?: EventTarget | null;
}

const SEEK_STEP_MS = 5000;

function isTyping(t: EventTarget | null | undefined): boolean {
  if (!(t instanceof HTMLElement)) return false;
  if (t.isContentEditable || t instanceof HTMLTextAreaElement) return true;
  return t instanceof HTMLInputElement && !['range', 'checkbox', 'radio', 'button'].includes(t.type);
}

/** Roles whose widgets move with the arrow keys themselves (a tab list, a menu, a listbox, a slider...). */
const ARROW_ROLES = new Set(['slider', 'tab', 'tablist', 'option', 'listbox', 'menu', 'menuitem', 'menuitemradio', 'menuitemcheckbox', 'radio', 'radiogroup', 'combobox', 'spinbutton', 'scrollbar', 'tree', 'treeitem', 'grid', 'gridcell']);

/** Widgets that use the arrow keys: sliders, selects, text fields, radio buttons and anything with an arrow-key role. */
function usesArrows(t: EventTarget | null | undefined): boolean {
  if (!(t instanceof HTMLElement)) return false;
  if (isTyping(t) || t instanceof HTMLSelectElement) return true;
  if (t instanceof HTMLInputElement && (t.type === 'range' || t.type === 'radio')) return true;
  const role = t.getAttribute('role');
  if (role && ARROW_ROLES.has(role)) return true;
  return !!t.closest('[role="tablist"], [role="menu"], [role="listbox"], [role="radiogroup"]');
}

/** Controls that Space activates (a button, a switch, a select...): Space belongs to them. */
function activatesOnSpace(t: EventTarget | null | undefined): boolean {
  if (!(t instanceof HTMLElement)) return false;
  if (t instanceof HTMLButtonElement || t instanceof HTMLSelectElement || t.tagName === 'SUMMARY') return true;
  if (t instanceof HTMLInputElement) return t.type !== 'text';
  const role = t.getAttribute('role');
  return role === 'button' || role === 'switch' || role === 'checkbox' || role === 'tab' || role === 'menuitem' || role === 'option' || role === 'radio';
}

/** A row or tile whose click plays a song: after a mouse click on it, Space is "play/pause", not "click again". */
const PLAYS_ON_CLICK = '.trow-main, .row-edit-main, .cover-tile';
export function isRowButton(t: EventTarget | null | undefined): boolean {
  return t instanceof HTMLElement && !!t.closest(PLAYS_ON_CLICK);
}

/** Maps an in-app key press to what should happen. Global media keys are handled by the OS integration later. */
export function actionForKey(e: KeyLike, ctx: KeyContext): KeyAction {
  if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 'k') return ctx.nowPlayingOpen ? null : { kind: 'search' };
  if (e.ctrlKey || e.altKey || e.metaKey) return null;
  if (e.key === 'F11') return { kind: 'fullscreen' }; // works while typing too
  if (e.key === 'Escape') return ctx.nowPlayingOpen ? { kind: 'close' } : null;
  if (isTyping(e.target)) return null;
  if (e.key === '/') return ctx.nowPlayingOpen ? null : { kind: 'search' };
  if (!ctx.hasTrack) return null;
  switch (e.key) {
    case ' ':
      // A focused button keeps Space (that is how a keyboard user presses it), except a song row that the mouse just
      // clicked: it only has focus because of the click, and there Space means play/pause.
      if (activatesOnSpace(e.target) && !(ctx.pointerFocus && isRowButton(e.target))) return null;
      return { kind: 'command', command: { type: 'toggle' } };
    case 'ArrowRight':
    case 'ArrowLeft': {
      if (usesArrows(e.target)) return null; // sliders, selects, text fields and tab lists use arrows themselves
      const delta = e.key === 'ArrowRight' ? SEEK_STEP_MS : -SEEK_STEP_MS;
      const max = ctx.durationMs ?? Number.MAX_SAFE_INTEGER;
      return { kind: 'command', command: { type: 'seek', ms: Math.min(max, Math.max(0, ctx.positionMs + delta)) } };
    }
    case 'n':
    case 'N':
      return { kind: 'command', command: { type: 'next' } };
    case 'p':
    case 'P':
      return { kind: 'command', command: { type: 'previous' } };
    default:
      return null;
  }
}
