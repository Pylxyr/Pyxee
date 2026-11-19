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

/**
 * What the time rail of a snack shows. `ms` is how long the snack still has to live from now; `seq` changes each time
 * the timer is armed again; `fresh` says the rail starts full (a new snack, or the same message again) and not from
 * where it was (a snack that runs on after being held); `held` is a snack whose timer is paused.
 */
export interface Rail {
  ms: number;
  seq: number;
  fresh: boolean;
  held: boolean;
}

const MAX_VISIBLE = 3;

/** In-page notifications with optional Undo. Replaces native alert/confirm dialogs. */
export class SnackbarStore {
  items = $state<Snack[]>([]);
  /** The timer of each snack, as the rail along its foot shows it. */
  rails = $state<Record<number, Rail>>({});
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

  #arm(id: number, ms: number, fresh = true) {
    this.rails = { ...this.rails, [id]: { ms, seq: (this.rails[id]?.seq ?? 0) + 1, fresh, held: false } };
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
    const rail = this.rails[id];
    if (rail) this.rails = { ...this.rails, [id]: { ...rail, held: true } };
  }

  /** Lets a paused snack run out the time it had left (at least a second). */
  resume(id: number) {
    const ms = this.#left.get(id);
    if (ms === undefined || !this.items.some((s) => s.id === id)) return;
    this.#arm(id, ms, false);
  }

  dismiss(id: number) {
    clearTimeout(this.#timers.get(id));
    this.#timers.delete(id);
    this.#left.delete(id);
    this.#due.delete(id);
    if (id in this.rails) {
      const rails = { ...this.rails };
      delete rails[id];
      this.rails = rails;
    }
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
