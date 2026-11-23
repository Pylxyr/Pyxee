import { describe, expect, it } from 'vitest';
import { hscroll, keepInRow } from './scrollrow';

/** A row and an item with the given horizontal edges (jsdom has no layout, so the boxes are set by hand). */
function boxes(row: [number, number], item: [number, number]) {
  const r = document.createElement('div');
  const i = document.createElement('button');
  r.getBoundingClientRect = () => ({ left: row[0], right: row[1] }) as DOMRect;
  i.getBoundingClientRect = () => ({ left: item[0], right: item[1] }) as DOMRect;
  return { r, i };
}

describe('keeping the chosen tab or chip in its sideways row', () => {
  it('leaves the row alone when the item is already in view', () => {
    const { r, i } = boxes([0, 300], [100, 180]);
    r.scrollLeft = 40;
    keepInRow(r, i);
    expect(r.scrollLeft).toBe(40);
  });
  it('scrolls right, with a little room, when the item is cut off on the right', () => {
    const { r, i } = boxes([0, 300], [260, 360]);
    keepInRow(r, i);
    expect(r.scrollLeft).toBe(68);
  });
  it('scrolls left, with a little room, when the item is cut off on the left', () => {
    const { r, i } = boxes([20, 320], [-30, 60]);
    r.scrollLeft = 200;
    keepInRow(r, i);
    expect(r.scrollLeft).toBe(200 - 50 - 8);
  });
  it('does nothing without an item', () => {
    const { r } = boxes([0, 300], [0, 0]);
    expect(() => keepInRow(r, null)).not.toThrow();
  });
});

/** A row 300 wide whose content is 400 wide (jsdom has no layout). */
function wideRow(scrollWidth = 400) {
  const row = document.createElement('div');
  Object.defineProperty(row, 'clientWidth', { value: 300 });
  Object.defineProperty(row, 'scrollWidth', { value: scrollWidth });
  document.body.append(row);
  return row;
}

describe('a row wider than its window', () => {
  it('fades at the edge that has more behind it, and not at all when it fits', () => {
    const row = wideRow();
    const action = hscroll(row);
    expect(row.hasAttribute('data-fade')).toBe(true);
    expect(row.style.getPropertyValue('--fade-start')).toBe('0px');
    expect(row.style.getPropertyValue('--fade-end')).toBe('28px');
    row.scrollLeft = 100; // the end
    row.dispatchEvent(new Event('scroll'));
    expect(row.style.getPropertyValue('--fade-start')).toBe('28px');
    expect(row.style.getPropertyValue('--fade-end')).toBe('0px');
    action.destroy();
    const fits = wideRow(300);
    const fitsAction = hscroll(fits);
    expect(fits.hasAttribute('data-fade')).toBe(false);
    fitsAction.destroy();
  });

  it('turns the mouse wheel into a sideways scroll until the end, then gives the wheel back to the page', () => {
    const row = wideRow();
    const action = hscroll(row);
    const down = new WheelEvent('wheel', { deltaY: 60, cancelable: true });
    row.dispatchEvent(down);
    expect(down.defaultPrevented).toBe(true);
    expect(row.scrollLeft).toBe(60);
    row.scrollLeft = 100; // at the end: the page scrolls
    const more = new WheelEvent('wheel', { deltaY: 60, cancelable: true });
    row.dispatchEvent(more);
    expect(more.defaultPrevented).toBe(false);
    // A sideways gesture (a trackpad) is already handled by the browser.
    row.scrollLeft = 10;
    const sideways = new WheelEvent('wheel', { deltaX: 40, deltaY: 2, cancelable: true });
    row.dispatchEvent(sideways);
    expect(sideways.defaultPrevented).toBe(false);
    action.destroy();
  });

  it('leaves the wheel alone when the row fits', () => {
    const row = wideRow(300);
    const action = hscroll(row);
    const e = new WheelEvent('wheel', { deltaY: 60, cancelable: true });
    row.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
    action.destroy();
  });
});
