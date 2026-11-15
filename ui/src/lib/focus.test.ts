import { describe, expect, it } from 'vitest';
import { focusNeighbour, focusables } from './focus';

function root(html: string): HTMLElement {
  const el = document.createElement('div');
  el.innerHTML = html;
  document.body.append(el);
  return el;
}
const ids = (els: HTMLElement[]) => els.map((e) => e.id);

describe('focusables', () => {
  it('lists what Tab reaches, in order, and nothing else', () => {
    const r = root(`
      <button id="a">a</button>
      <button id="off" disabled>off</button>
      <button id="skip" tabindex="-1">skip</button>
      <div id="scroll" tabindex="0">scroll</div>
      <input id="hid" type="hidden" />
      <div hidden><button id="inhidden">x</button></div>
      <a id="link" href="#x">l</a>
      <input id="in" />`);
    expect(ids(focusables(r))).toEqual(['a', 'scroll', 'link', 'in']);
  });

  it('reaches only the summary of a closed details, and everything in an open one', () => {
    const r = root(`
      <details id="closed"><summary id="s1">more</summary><textarea id="t1"></textarea><button id="b1">go</button></details>
      <details id="open" open><summary id="s2">more</summary><textarea id="t2"></textarea></details>
      <button id="last">last</button>`);
    expect(ids(focusables(r))).toEqual(['s1', 's2', 't2', 'last']);
  });
});

describe('focusNeighbour', () => {
  const list = (n: number) => {
    document.body.innerHTML = '';
    return root(`<div data-scroll tabindex="0" id="page"><ul>${Array.from({ length: n }, (_, i) => `<li id="r${i}"><button class="row-edit-main" id="m${i}">m</button><button id="x${i}">x</button></li>`).join('')}</ul></div>`);
  };

  it('moves focus to the next row, or the previous one for the last row', () => {
    list(3);
    expect(focusNeighbour(document.getElementById('x0'))).toBe(true);
    expect(document.activeElement?.id).toBe('m1');
    expect(focusNeighbour(document.getElementById('x2'))).toBe(true);
    expect(document.activeElement?.id).toBe('m1');
  });

  it('falls back to the scrolling page when the list will be empty', () => {
    list(1);
    expect(focusNeighbour(document.getElementById('x0'))).toBe(true);
    expect(document.activeElement?.id).toBe('page');
  });

  it('does nothing for something that is not in a row', () => {
    expect(focusNeighbour(null)).toBe(false);
  });
});
