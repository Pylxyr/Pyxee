/**
 * Drag-to-reorder with pointer events on a handle (or, with `threshold`, on the whole row). HTML5 drag and drop is not used: in a Tauri
 * window on Windows it can be swallowed by the webview's own file-drop handling, while pointer
 * events behave the same everywhere (mouse, pen and touch).
 */

/** Where the dragged row ends up: how many OTHER rows have their midpoint above `y`. */
export function dropIndex(mids: number[], dragged: number, y: number): number {
  let n = 0;
  mids.forEach((m, i) => {
    if (i !== dragged && m < y) n++;
  });
  return n;
}

export interface ReorderParams {
  /** Current index of the row this handle belongs to. */
  index: number;
  /** All draggable rows of the list, in order. */
  rows: () => HTMLElement[];
  /** Called while dragging with the index the row would land at, and with null when it ends. */
  onover: (to: number | null) => void;
  /** Called once on release if the row ended up somewhere else. */
  ondrop: (to: number) => void;
  /**
   * For a row that is itself a button (a click plays the song) and has no handle: the drag starts only once the pointer has
   * moved this many pixels, and the click that follows a drag is dropped. A press that is not moved is an ordinary click.
   * A finger is left to scroll the list (it reorders with the Queue page's handles). Without it, a press on the handle
   * starts the drag at once.
   */
  threshold?: number;
}

function scrollParent(el: HTMLElement | null): HTMLElement | null {
  for (let n = el?.parentElement ?? null; n; n = n.parentElement) {
    const o = getComputedStyle(n).overflowY;
    if ((o === 'auto' || o === 'scroll') && n.scrollHeight > n.clientHeight) return n;
  }
  return null;
}

const EDGE = 48; // px from the top/bottom of the scrolling area where dragging scrolls it
const STEP = 14;

export function reorder(handle: HTMLElement, initial: ReorderParams) {
  let p = initial;
  let pointer: number | null = null;
  let to = 0;

  const finish = (commit: boolean) => {
    if (pointer === null) return;
    try {
      handle.releasePointerCapture(pointer);
    } catch {
      /* already released */
    }
    pointer = null;
    window.removeEventListener('keydown', onkey, true);
    const target = to;
    p.onover(null);
    if (commit && target !== p.index) p.ondrop(target);
  };
  const onkey = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      finish(false);
    }
  };
  // `threshold` mode: a press that has not moved far enough yet.
  let armed: { id: number; x: number; y: number } | null = null;
  let swallow = false;

  const begin = (id: number) => {
    pointer = id;
    to = p.index;
    handle.setPointerCapture(id);
    window.addEventListener('keydown', onkey, true);
    p.onover(to);
  };
  const down = (e: PointerEvent) => {
    if (pointer !== null || (e.pointerType === 'mouse' && e.button !== 0)) return;
    if (p.threshold) {
      if (e.pointerType === 'touch') return;
      armed = { id: e.pointerId, x: e.clientX, y: e.clientY };
      return;
    }
    e.preventDefault();
    begin(e.pointerId);
  };
  // A drag that ends on a button would click it: the row's own play button must not fire.
  const click = (e: MouseEvent) => {
    if (!swallow) return;
    swallow = false;
    e.stopPropagation();
    e.preventDefault();
  };
  // The browser's own picture drag (a cover image) would take over the pointer.
  const nativeDrag = (e: Event) => {
    if (p.threshold) e.preventDefault();
  };
  const move = (e: PointerEvent) => {
    if (armed && e.pointerId === armed.id && pointer === null) {
      if (Math.hypot(e.clientX - armed.x, e.clientY - armed.y) < (p.threshold ?? 0)) return;
      armed = null;
      begin(e.pointerId);
    }
    if (e.pointerId !== pointer) return;
    const rows = p.rows();
    const next = dropIndex(
      rows.map((r) => {
        const b = r.getBoundingClientRect();
        return b.top + b.height / 2;
      }),
      p.index,
      e.clientY,
    );
    const sp = scrollParent(handle);
    if (sp) {
      const b = sp.getBoundingClientRect();
      if (e.clientY < b.top + EDGE) sp.scrollBy(0, -STEP);
      else if (e.clientY > b.bottom - EDGE) sp.scrollBy(0, STEP);
    }
    if (next !== to) {
      to = next;
      p.onover(to);
    }
  };
  const up = (e: PointerEvent) => {
    if (armed && e.pointerId === armed.id) armed = null; // not moved far enough: a click
    if (e.pointerId !== pointer) return;
    if (p.threshold) {
      swallow = true;
      setTimeout(() => (swallow = false), 0); // the click comes right after the release, or not at all
    }
    finish(true);
  };
  const cancel = (e: PointerEvent) => {
    if (armed && e.pointerId === armed.id) armed = null;
    if (e.pointerId === pointer) finish(false);
  };

  handle.addEventListener('pointerdown', down);
  handle.addEventListener('pointermove', move);
  handle.addEventListener('pointerup', up);
  handle.addEventListener('pointercancel', cancel);
  handle.addEventListener('click', click, true);
  handle.addEventListener('dragstart', nativeDrag);
  return {
    update(next: ReorderParams) {
      p = next;
    },
    destroy() {
      finish(false);
      handle.removeEventListener('pointerdown', down);
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
      handle.removeEventListener('pointercancel', cancel);
      handle.removeEventListener('click', click, true);
      handle.removeEventListener('dragstart', nativeDrag);
    },
  };
}
