import { afterEach, describe, expect, it, vi } from 'vitest';
import { dropIndex, reorder } from './reorder';

describe('dropIndex', () => {
  const mids = [20, 60, 100, 140];
  it('counts the other rows above the pointer', () => {
    expect(dropIndex(mids, 0, 0)).toBe(0);
    expect(dropIndex(mids, 0, 70)).toBe(1); // past row 1's middle (row 0 itself is not counted)
    expect(dropIndex(mids, 0, 500)).toBe(3);
    expect(dropIndex(mids, 3, 0)).toBe(0);
    expect(dropIndex(mids, 3, 65)).toBe(2);
  });
});

describe('reorder (pointer dragging on a handle)', () => {
  const cleanup: Array<() => void> = [];
  afterEach(() => {
    cleanup.splice(0).forEach((f) => f());
    document.body.innerHTML = '';
  });

  function setup(index: number) {
    const rows = [0, 1, 2].map((i) => {
      const el = document.createElement('li');
      el.getBoundingClientRect = () => ({ top: i * 40, height: 40, bottom: i * 40 + 40 }) as DOMRect;
      document.body.append(el);
      return el;
    });
    const handle = document.createElement('span');
    handle.setPointerCapture = vi.fn();
    handle.releasePointerCapture = vi.fn();
    rows[index]!.append(handle);
    const over = vi.fn();
    const drop = vi.fn();
    const action = reorder(handle, { index, rows: () => rows, onover: over, ondrop: drop });
    cleanup.push(() => action.destroy());
    const fire = (type: string, y = 0) =>
      handle.dispatchEvent(new PointerEvent(type, { pointerId: 1, clientY: y, button: 0, pointerType: 'mouse', bubbles: true, cancelable: true }));
    return { fire, over, drop, handle };
  }

  it('dragging a row down past the next rows reports where it would land and drops there once', () => {
    const { fire, over, drop, handle } = setup(0);
    fire('pointerdown', 20);
    expect(handle.setPointerCapture).toHaveBeenCalledWith(1);
    fire('pointermove', 70);
    fire('pointermove', 110);
    expect(over).toHaveBeenLastCalledWith(2);
    fire('pointerup', 110);
    expect(drop).toHaveBeenCalledExactlyOnceWith(2);
    expect(over).toHaveBeenLastCalledWith(null);
  });

  it('dragging up works too', () => {
    const { fire, drop } = setup(2);
    fire('pointerdown', 100);
    fire('pointermove', 5);
    fire('pointerup', 5);
    expect(drop).toHaveBeenCalledExactlyOnceWith(0);
  });

  it('releasing where it started changes nothing', () => {
    const { fire, drop } = setup(1);
    fire('pointerdown', 60);
    fire('pointermove', 62);
    fire('pointerup', 62);
    expect(drop).not.toHaveBeenCalled();
  });

  it('Escape cancels the drag', () => {
    const { fire, over, drop } = setup(0);
    fire('pointerdown', 20);
    fire('pointermove', 110);
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    fire('pointerup', 110);
    expect(drop).not.toHaveBeenCalled();
    expect(over).toHaveBeenLastCalledWith(null);
  });

  it('a right click does not start a drag', () => {
    const { handle, over } = setup(0);
    handle.dispatchEvent(new PointerEvent('pointerdown', { pointerId: 1, button: 2, pointerType: 'mouse', bubbles: true }));
    expect(over).not.toHaveBeenCalled();
  });
});

describe('reorder on a whole row (threshold)', () => {
  const cleanup: Array<() => void> = [];
  afterEach(() => {
    cleanup.splice(0).forEach((f) => f());
    document.body.innerHTML = '';
    vi.useRealTimers();
  });

  function setup(index: number, pointerType = 'mouse') {
    const rows = [0, 1, 2].map((i) => {
      const el = document.createElement('li');
      el.getBoundingClientRect = () => ({ top: i * 40, height: 40, bottom: i * 40 + 40 }) as DOMRect;
      el.setPointerCapture = vi.fn();
      el.releasePointerCapture = vi.fn();
      document.body.append(el);
      return el;
    });
    const row = rows[index]!;
    const button = document.createElement('button');
    row.append(button);
    const over = vi.fn();
    const drop = vi.fn();
    const clicked = vi.fn();
    row.addEventListener('click', clicked); // stands for the row's own play button
    const action = reorder(row, { index, rows: () => rows, onover: over, ondrop: drop, threshold: 5 });
    cleanup.push(() => action.destroy());
    const fire = (type: string, y = 0, x = 0) =>
      row.dispatchEvent(new PointerEvent(type, { pointerId: 1, clientX: x, clientY: y, button: 0, pointerType, bubbles: true, cancelable: true }));
    return { fire, over, drop, clicked, row, button };
  }

  it('a press that does not move is a click: nothing is dragged and the click goes through', () => {
    const { fire, over, drop, clicked, row, button } = setup(0);
    fire('pointerdown', 20);
    fire('pointermove', 22);
    fire('pointerup', 22);
    button.click();
    expect(over).not.toHaveBeenCalled();
    expect(drop).not.toHaveBeenCalled();
    expect(row.setPointerCapture).not.toHaveBeenCalled();
    expect(clicked).toHaveBeenCalledTimes(1);
  });

  it('starts dragging only after the pointer moved the threshold, then drops once where it ended', () => {
    const { fire, over, drop, row } = setup(0);
    fire('pointerdown', 20);
    fire('pointermove', 23);
    expect(over).not.toHaveBeenCalled();
    fire('pointermove', 30);
    expect(row.setPointerCapture).toHaveBeenCalledWith(1);
    expect(over).toHaveBeenCalledWith(0);
    fire('pointermove', 110);
    expect(over).toHaveBeenLastCalledWith(2);
    fire('pointerup', 110);
    expect(drop).toHaveBeenCalledExactlyOnceWith(2);
    expect(over).toHaveBeenLastCalledWith(null);
  });

  it('drops the click that ends a drag (it would play the row), but not the next one', () => {
    vi.useFakeTimers();
    const { fire, clicked, button } = setup(0);
    fire('pointerdown', 20);
    fire('pointermove', 90);
    fire('pointerup', 90);
    button.click();
    expect(clicked).not.toHaveBeenCalled();
    vi.runAllTimers();
    button.click();
    expect(clicked).toHaveBeenCalledTimes(1);
  });

  it('leaves a finger to scroll the list', () => {
    const { fire, over, row } = setup(0, 'touch');
    fire('pointerdown', 20);
    fire('pointermove', 90);
    expect(over).not.toHaveBeenCalled();
    expect(row.setPointerCapture).not.toHaveBeenCalled();
  });

  it('Escape cancels a drag without dropping', () => {
    const { fire, drop, over } = setup(0);
    fire('pointerdown', 20);
    fire('pointermove', 90);
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    fire('pointerup', 90);
    expect(drop).not.toHaveBeenCalled();
    expect(over).toHaveBeenLastCalledWith(null);
  });
});
