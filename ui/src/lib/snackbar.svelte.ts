import { markListChange } from './motion';

export interface SnackAction {
  label: string;
  run: () => void;
}
export interface Snack {
  id: number;
  message: string;
  action?: SnackAction;
}

const MAX_VISIBLE = 3;

/** In-page notifications with optional Undo. Replaces native alert/confirm dialogs. */
export class SnackbarStore {
  items = $state<Snack[]>([]);
  #next = 1;
  #timers = new Map<number, ReturnType<typeof setTimeout>>();
  /** For a snack whose timer is paused (pointer over it, focus inside it): how long it still has to show. */
  #left = new Map<number, number>();
  #due = new Map<number, { at: number; ms: number }>();

  /**
   * Shows a message. The same message with no action that is already showing is not added a second time: its timer
   * starts again and its id is returned (a burst of identical failures reads as one notice, not a stack of them).
   */
  push(message: string, opts: { action?: SnackAction; timeoutMs?: number } = {}): number {
    if (!opts.action) {
      const same = this.items.find((s) => s.message === message && !s.action);
      if (same) {
        this.#arm(same.id, opts.timeoutMs ?? 4000);
        return same.id;
      }
    }
    const id = this.#next++;
    this.items = [...this.items, { id, message, action: opts.action }].slice(-MAX_VISIBLE);
    this.#arm(id, opts.timeoutMs ?? (opts.action ? 7000 : 4000));
    return id;
  }

  #arm(id: number, ms: number) {
    clearTimeout(this.#timers.get(id));
    this.#left.delete(id);
    this.#due.set(id, { at: Date.now(), ms });
    this.#timers.set(
      id,
      setTimeout(() => this.dismiss(id), ms),
    );
  }

  /** Holds the snack on screen (the pointer is over it, or focus is inside it) so it cannot vanish under a click. */
  pause(id: number) {
    const due = this.#due.get(id);
    if (!due || !this.#timers.has(id)) return;
    clearTimeout(this.#timers.get(id));
    this.#timers.delete(id);
    this.#left.set(id, Math.max(1000, due.ms - (Date.now() - due.at)));
  }

  /** Lets a paused snack run out the time it had left (at least a second). */
  resume(id: number) {
    const ms = this.#left.get(id);
    if (ms === undefined || !this.items.some((s) => s.id === id)) return;
    this.#arm(id, ms);
  }

  dismiss(id: number) {
    clearTimeout(this.#timers.get(id));
    this.#timers.delete(id);
    this.#left.delete(id);
    this.#due.delete(id);
    this.items = this.items.filter((s) => s.id !== id);
  }

  /** Runs the snack's action (e.g. Undo) and closes it. */
  act(id: number) {
    const s = this.items.find((x) => x.id === id);
    this.dismiss(id);
    // An Undo brings a row back: let it arrive with its own motion.
    markListChange();
    s?.action?.run();
  }
}

export const snackbar = new SnackbarStore();
