/** What Tab can reach inside `root`, in order: controls that are enabled, not taken out of the tab order
 *  (`tabindex="-1"`), not hidden, and not inside a closed `<details>` (its `<summary>` is still reachable). */
const SELECTOR = 'button, [href], input, select, textarea, summary, [tabindex], [contenteditable=""], [contenteditable="true"]';

export function focusables(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(SELECTOR)].filter((el) => {
    if (el.hasAttribute('disabled') || el.tabIndex < 0 || el.closest('[hidden], [inert]')) return false;
    if (el instanceof HTMLInputElement && el.type === 'hidden') return false;
    // Inside a closed <details> only its own summary can be reached.
    for (let d = el.closest('details'); d && root.contains(d); d = d.parentElement?.closest('details') ?? null) {
      if (!d.open && !(el.tagName === 'SUMMARY' && el.parentElement === d)) return false;
    }
    return true;
  });
}

/** The main button of a list row (a track row, a queue row, a cover tile). */
const ROW_MAIN = '.trow-main, .row-edit-main, .cover-tile';

/**
 * Called as a row is about to be removed: moves focus to the row after it (or before it, for the last one), or to the
 * scrolling page when the list will be empty, so keyboard focus is not dropped onto the document when the row's own
 * button disappears. Returns whether focus was moved.
 */
export function focusNeighbour(from: Element | null | undefined): boolean {
  const row = from?.closest('li');
  if (!row) return false;
  for (const sib of [row.nextElementSibling, row.previousElementSibling]) {
    const main = sib?.querySelector<HTMLElement>(ROW_MAIN);
    if (main) {
      main.focus({ preventScroll: true });
      return true;
    }
  }
  const page = row.closest<HTMLElement>('[data-scroll]');
  if (page) {
    page.focus({ preventScroll: true });
    return true;
  }
  return false;
}
