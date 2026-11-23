/**
 * Brings `item` into view inside the sideways-scrolling `row`, moving only that row. (`scrollIntoView` would also scroll
 * every page around it, which slides a whole window's content sideways.)
 */
export function keepInRow(row: HTMLElement, item: HTMLElement | null): void {
  if (!item) return;
  const r = row.getBoundingClientRect();
  const i = item.getBoundingClientRect();
  if (i.left < r.left) row.scrollLeft += i.left - r.left - 8;
  else if (i.right > r.right) row.scrollLeft += i.right - r.right + 8;
}
