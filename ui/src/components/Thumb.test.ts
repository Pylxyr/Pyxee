import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';
import Thumb from './Thumb.svelte';
import { viewport } from '$lib/viewport.svelte';

afterEach(() => vi.useRealTimers());

describe('Thumb after the window changes', () => {
  it('a cover that never loaded is requested again (a fresh <img>) after a fullscreen switch', async () => {
    const { container } = render(Thumb, { src: 'https://i.ytimg.com/vi/abc/hqdefault.jpg', seed: 'k', size: 352, fill: true });
    const first = container.querySelector('img');
    expect(first).not.toBeNull();
    viewport.bump();
    await Promise.resolve();
    await new Promise((r) => setTimeout(r, 0));
    const second = container.querySelector('img');
    expect(second).not.toBeNull();
    expect(second).not.toBe(first);
  });

  it('a cover that did load is left alone', async () => {
    const { container } = render(Thumb, { src: 'https://i.ytimg.com/vi/abc/hqdefault.jpg', seed: 'k', size: 352, fill: true });
    const first = container.querySelector('img')!;
    await fireEvent.load(first);
    viewport.bump();
    await new Promise((r) => setTimeout(r, 0));
    expect(container.querySelector('img')).toBe(first);
  });

  it('after every candidate failed during the change, it starts over from the first', async () => {
    const { container } = render(Thumb, { src: 'https://i.ytimg.com/vi/abc/hqdefault.jpg', seed: 'k', size: 352, fill: true });
    for (let i = 0; i < 4; i++) {
      const img = container.querySelector('img');
      if (!img) break;
      await fireEvent.error(img);
    }
    expect(container.querySelector('img')).toBeNull();
    viewport.bump();
    await new Promise((r) => setTimeout(r, 0));
    expect(container.querySelector('img')).not.toBeNull();
  });
});
