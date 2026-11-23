import { describe, expect, it } from 'vitest';
import { keepInRow } from './scrollrow';

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
