import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import Wash from './Wash.svelte';
import MixCard from './MixCard.svelte';
import { look } from '$lib/look.svelte';

afterEach(() => {
  look.reset();
  vi.useRealTimers();
});

const layers = (c: HTMLElement) => [...c.querySelectorAll<HTMLElement>('.wash')];

describe('Wash', () => {
  it('is one decorative layer that carries its own colors', () => {
    const { container } = render(Wash, { id: 'a', look: 'gradient', class: 'hero-wash', style: '--hero-a: #112233' });
    const [layer, ...rest] = layers(container);
    expect(rest).toEqual([]);
    expect(layer!.classList.contains('hero-wash')).toBe(true);
    expect(layer!.getAttribute('data-wash')).toBe('gradient');
    expect(layer!.getAttribute('aria-hidden')).toBe('true');
    expect(layer!.getAttribute('style')).toContain('#112233');
  });

  it('new colors are a new layer on top; the old one stays under it until it is covered, then goes', async () => {
    vi.useFakeTimers();
    const { container, rerender } = render(Wash, { id: 'a', look: 'aurora', style: '--hero-a: #111111' });
    await rerender({ id: 'b', look: 'aurora', style: '--hero-a: #eeeeee' });
    const both = layers(container);
    expect(both.length).toBe(2);
    expect(both[0]!.getAttribute('style')).toContain('#111111'); // the old one keeps its own colors, it is not recolored under the fade
    expect(both[1]!.getAttribute('style')).toContain('#eeeeee'); // later in the document: on top
    await vi.advanceTimersByTimeAsync(1500);
    const left = layers(container);
    expect(left.length).toBe(1);
    expect(left[0]!.getAttribute('style')).toContain('#eeeeee');
  });

  it('the same id is the same layer: other changes do not stack another', async () => {
    const { container, rerender } = render(Wash, { id: 'a', look: 'vinyl' });
    await rerender({ id: 'a', look: 'vinyl', class: 'x' });
    expect(layers(container).length).toBe(1);
  });
});

describe('the Tune in card', () => {
  const track = { key: 'abc123', title: 'Song', artist: 'Band', durationMs: 1000, thumbnail: null };

  it('has its gradient as a layer with the stand-in colors, and the Surprise card has none', () => {
    const { container } = render(MixCard, { track, onpick: () => {} });
    const [layer] = layers(container);
    expect(layer!.classList.contains('mix-bg')).toBe(true);
    expect(layer!.getAttribute('style')).toMatch(/--mix-a: hsl\(\d+ 68% 34%\); --mix-b: hsl\(\d+ 72% 24%\)/);
    const surprise = render(MixCard, { track: null, onpick: () => {} });
    expect(layers(surprise.container).length).toBe(0);
  });

  it('takes the theme colors, not the stand-in, when the theme color is chosen', () => {
    look.set('tune', 'theme');
    const { container } = render(MixCard, { track, onpick: () => {} });
    expect(layers(container)[0]!.getAttribute('style') ?? '').not.toContain('--mix-a');
  });
});
