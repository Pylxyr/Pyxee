import type { CommandDto } from './bindings/CommandDto';

export type KeyAction = { kind: 'command'; command: CommandDto } | { kind: 'close' } | null;

export interface KeyContext {
  positionMs: number;
  durationMs: number | null;
  nowPlayingOpen: boolean;
  hasTrack: boolean;
}

interface KeyLike {
  key: string;
  ctrlKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  target?: EventTarget | null;
}

const SEEK_STEP_MS = 5000;

function isTyping(t: EventTarget | null | undefined): boolean {
  if (!(t instanceof HTMLElement)) return false;
  if (t.isContentEditable || t instanceof HTMLTextAreaElement) return true;
  return t instanceof HTMLInputElement && !['range', 'checkbox', 'radio', 'button'].includes(t.type);
}

function isControl(t: EventTarget | null | undefined): boolean {
  return t instanceof HTMLButtonElement || t instanceof HTMLAnchorElement || t instanceof HTMLSelectElement ||
    (t instanceof HTMLInputElement && !isTyping(t));
}

/** Maps an in-app key press to what should happen. Global media keys are handled by the OS integration later. */
export function actionForKey(e: KeyLike, ctx: KeyContext): KeyAction {
  if (e.ctrlKey || e.altKey || e.metaKey) return null;
  if (e.key === 'Escape') return ctx.nowPlayingOpen ? { kind: 'close' } : null;
  if (isTyping(e.target) || !ctx.hasTrack) return null;
  switch (e.key) {
    case ' ':
      return isControl(e.target) ? null : { kind: 'command', command: { type: 'toggle' } };
    case 'ArrowRight':
    case 'ArrowLeft': {
      if (isControl(e.target)) return null; // sliders and buttons use arrows themselves
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
