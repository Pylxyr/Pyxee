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

  push(message: string, opts: { action?: SnackAction; timeoutMs?: number } = {}): number {
    const id = this.#next++;
    this.items = [...this.items, { id, message, action: opts.action }].slice(-MAX_VISIBLE);
    this.#timers.set(
      id,
      setTimeout(() => this.dismiss(id), opts.timeoutMs ?? (opts.action ? 7000 : 4000)),
    );
    return id;
  }

  dismiss(id: number) {
    clearTimeout(this.#timers.get(id));
    this.#timers.delete(id);
    this.items = this.items.filter((s) => s.id !== id);
  }

  /** Runs the snack's action (e.g. Undo) and closes it. */
  act(id: number) {
    const s = this.items.find((x) => x.id === id);
    this.dismiss(id);
    s?.action?.run();
  }
}

export const snackbar = new SnackbarStore();
