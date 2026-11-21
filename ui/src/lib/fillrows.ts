/**
 * Svelte action for a grid of cards: shows only as many as fill whole rows (with fewer than a row, or a single scrolling
 * row, all of them). It looks again when the grid's width or its children change, never otherwise, so it costs nothing
 * while the window stays as it is.
 */
export function fillRows(node: HTMLElement): { destroy(): void } {
  const apply = () => {
    const items = Array.from(node.children) as HTMLElement[];
    const tracks = getComputedStyle(node).gridTemplateColumns;
    const cols = tracks && tracks !== 'none' ? tracks.split(/\s+/).filter(Boolean).length : 0;
    const shown = cols === 0 || items.length <= cols ? items.length : Math.floor(items.length / cols) * cols;
    items.forEach((el, i) => {
      el.hidden = i >= shown;
    });
  };
  apply();
  const resize = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(apply);
  resize?.observe(node);
  const children = typeof MutationObserver === 'undefined' ? null : new MutationObserver(apply);
  children?.observe(node, { childList: true });
  return {
    destroy() {
      resize?.disconnect();
      children?.disconnect();
    },
  };
}
