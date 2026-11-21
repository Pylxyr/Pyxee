import { describe, expect, it } from 'vitest';
import { fillRows } from './fillrows';

function grid(count: number, tracks: string) {
  const ul = document.createElement('ul');
  for (let i = 0; i < count; i++) ul.appendChild(document.createElement('li'));
  const real = window.getComputedStyle;
  window.getComputedStyle = ((el: Element) => (el === ul ? ({ gridTemplateColumns: tracks } as CSSStyleDeclaration) : real(el))) as typeof window.getComputedStyle;
  return { ul, restore: () => (window.getComputedStyle = real) };
}
const shown = (ul: HTMLElement) => Array.from(ul.children).filter((c) => !(c as HTMLElement).hidden).length;

describe('fillRows', () => {
  it('hides the cards that would leave a short last row', () => {
    const { ul, restore } = grid(9, '200px 200px 200px 200px');
    fillRows(ul);
    expect(shown(ul)).toBe(8);
    restore();
  });
  it('shows everything when there are fewer than one row, or one scrolling row', () => {
    const a = grid(3, '200px 200px 200px 200px');
    fillRows(a.ul);
    expect(shown(a.ul)).toBe(3);
    a.restore();
    const b = grid(9, 'none');
    fillRows(b.ul);
    expect(shown(b.ul)).toBe(9);
    b.restore();
  });
});
