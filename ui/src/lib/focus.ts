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
