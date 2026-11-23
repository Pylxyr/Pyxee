/**
 * Brings `item` into view inside the sideways-scrolling `row`, moving only that row. (`scrollIntoView` would also scroll
 * every page around it, which slides a whole window's content sideways.) `room` is how much of the next item beyond it
 * is shown too, so the row visibly goes on.
 */
export function keepInRow(row: HTMLElement, item: HTMLElement | null, room = 8): void {
  if (!item) return;
  const r = row.getBoundingClientRect();
  const i = item.getBoundingClientRect();
  if (i.left < r.left) row.scrollLeft += i.left - r.left - room;
  else if (i.right > r.right) row.scrollLeft += i.right - r.right + room;
}

/** How wide the fade at a row's cut-off edge is. */
const FADE = 28;

/**
 * Makes a row that is wider than its window (tabs, genre chips) usable without a touch screen, and shows that it goes on:
 * the mouse wheel scrolls it sideways, a mouse can drag it, and an edge that has more behind it fades out
 * (`data-fade`, `--fade-start`, `--fade-end`; see `.hscroll` in app.css). It does nothing for a row that fits.
 */
export function hscroll(row: HTMLElement) {
  function room() {
    return row.scrollWidth - row.clientWidth;
  }
  function measure() {
    const over = room() > 1;
    if (over) row.setAttribute('data-fade', '');
    else row.removeAttribute('data-fade');
    row.style.setProperty('--fade-start', over && row.scrollLeft > 2 ? `${FADE}px` : '0px');
    row.style.setProperty('--fade-end', over && row.scrollLeft < room() - 2 ? `${FADE}px` : '0px');
  }

  // A wheel turn (vertical) moves the row, until it is at that end: then the page gets it again.
  function wheel(e: WheelEvent) {
    if (room() <= 1 || e.ctrlKey || Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
    const next = row.scrollLeft + e.deltaY;
    if ((e.deltaY < 0 && row.scrollLeft <= 0) || (e.deltaY > 0 && row.scrollLeft >= room() - 1)) return;
    e.preventDefault();
    row.scrollLeft = next;
  }

  // Dragging with a mouse (a finger already scrolls it): past a few pixels it is a drag, and the click that ends it is dropped.
  let startX = 0;
  let startLeft = 0;
  let dragging = false;
  let down = false;
  function pointerdown(e: PointerEvent) {
    if (e.pointerType !== 'mouse' || e.button !== 0 || room() <= 1) return;
    down = true;
    dragging = false;
    startX = e.clientX;
    startLeft = row.scrollLeft;
  }
  function pointermove(e: PointerEvent) {
    if (!down) return;
    if (!dragging && Math.abs(e.clientX - startX) > 5) dragging = true;
    if (dragging) row.scrollLeft = startLeft - (e.clientX - startX);
  }
  function pointerup() {
    down = false;
  }
  function click(e: MouseEvent) {
    if (!dragging) return;
    dragging = false;
    e.stopPropagation();
    e.preventDefault();
  }

  row.addEventListener('wheel', wheel, { passive: false });
  row.addEventListener('pointerdown', pointerdown);
  window.addEventListener('pointermove', pointermove);
  window.addEventListener('pointerup', pointerup);
  row.addEventListener('click', click, true);
  row.addEventListener('scroll', measure, { passive: true });
  const watch = typeof ResizeObserver === 'function' ? new ResizeObserver(measure) : null;
  watch?.observe(row);
  // The row's own width can stay put while its items change (a chip row grows with "More genres").
  const items = typeof MutationObserver === 'function' ? new MutationObserver(measure) : null;
  items?.observe(row, { childList: true, subtree: true });
  measure();

  return {
    destroy() {
      row.removeEventListener('wheel', wheel);
      row.removeEventListener('pointerdown', pointerdown);
      window.removeEventListener('pointermove', pointermove);
      window.removeEventListener('pointerup', pointerup);
      row.removeEventListener('click', click, true);
      row.removeEventListener('scroll', measure);
      watch?.disconnect();
      items?.disconnect();
    },
  };
}
