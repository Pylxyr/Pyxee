import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';
import Thumb from './Thumb.svelte';
import { viewport } from '$lib/viewport.svelte';

afterEach(() => {
  vi.useRealTimers();
  window.dispatchEvent(new Event('online')); // leave the shared network state as it was found
});

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

async function failAll(container: HTMLElement) {
  for (let i = 0; i < 6; i++) {
    const img = container.querySelector('img');
    if (!img) return;
    await fireEvent.error(img);
  }
}

function loadWithSize(img: HTMLImageElement, width: number, height: number) {
  Object.defineProperty(img, 'naturalWidth', { value: width, configurable: true });
  Object.defineProperty(img, 'naturalHeight', { value: height, configurable: true });
  return fireEvent.load(img);
}

describe('Thumb and YouTube\'s grey "no picture" image', () => {
  it('a 120x90 still counts as missing and the next URL is tried', async () => {
    const { container } = render(Thumb, { src: 'https://i.ytimg.com/vi/nopic1/hqdefault.jpg', seed: 'k', size: 352, fill: true });
    const first = container.querySelector('img')!;
    expect(first.getAttribute('src')).toContain('maxresdefault');
    await loadWithSize(first, 120, 90);
    await new Promise((r) => setTimeout(r, 0));
    const next = container.querySelector('img')!;
    expect(next.getAttribute('src')).toContain('hq720');
    await loadWithSize(next, 1280, 720);
    expect(container.querySelector('img')).toBe(next);
  });

  it('a song with no real still ends on the placeholder, and another row skips what is known to be missing', async () => {
    const src = 'https://i.ytimg.com/vi/nopic2/hqdefault.jpg';
    const { container } = render(Thumb, { src, seed: 'k', size: 352, fill: true });
    for (let i = 0; i < 4; i++) {
      const img = container.querySelector('img');
      if (!img) break;
      await loadWithSize(img, 120, 90);
      await new Promise((r) => setTimeout(r, 0));
    }
    expect(container.querySelector('img')).toBeNull();
    const again = render(Thumb, { src, seed: 'k', size: 352, fill: true });
    expect(again.container.querySelector('img')).toBeNull();
  });

  it('a small picture from another host is a real picture', async () => {
    const { container } = render(Thumb, { src: 'https://example.com/a.jpg', seed: 'k', size: 44 });
    const img = container.querySelector('img')!;
    await loadWithSize(img, 100, 100);
    expect(container.querySelector('img')).toBe(img);
  });
});

describe('Thumb retries a cover that could not be fetched', () => {
  it('waits while offline without any timer, then retries once the network is back', async () => {
    vi.useFakeTimers();
    window.dispatchEvent(new Event('offline'));
    const { container } = render(Thumb, { src: 'https://i.ytimg.com/vi/off1/hqdefault.jpg', seed: 'k', size: 352, fill: true });
    await failAll(container);
    expect(container.querySelector('img')).toBeNull();
    await vi.advanceTimersByTimeAsync(120_000);
    expect(container.querySelector('img')).toBeNull(); // nothing was polled
    expect(vi.getTimerCount()).toBe(0);
    window.dispatchEvent(new Event('online'));
    await vi.advanceTimersByTimeAsync(2_100);
    expect(container.querySelector('img')).not.toBeNull();
  });

  it('retries at most three times while online, then stops', async () => {
    vi.useFakeTimers();
    const { container } = render(Thumb, { src: 'https://i.ytimg.com/vi/on1/hqdefault.jpg', seed: 'k', size: 352, fill: true });
    await failAll(container);
    for (let n = 0; n < 3; n++) {
      expect(container.querySelector('img')).toBeNull();
      await vi.advanceTimersByTimeAsync(31_000);
      expect(container.querySelector('img')).not.toBeNull();
      await failAll(container);
    }
    await vi.advanceTimersByTimeAsync(300_000);
    expect(container.querySelector('img')).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('a cover that YouTube says does not exist is not retried', async () => {
    vi.useFakeTimers();
    const { container } = render(Thumb, { src: 'https://i.ytimg.com/vi/nopic3/hqdefault.jpg', seed: 'k', size: 352, fill: true });
    for (let i = 0; i < 4; i++) {
      const img = container.querySelector('img');
      if (!img) break;
      await loadWithSize(img, 120, 90);
      await vi.advanceTimersByTimeAsync(0);
    }
    expect(container.querySelector('img')).toBeNull();
    await vi.advanceTimersByTimeAsync(300_000);
    expect(container.querySelector('img')).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  });
});
