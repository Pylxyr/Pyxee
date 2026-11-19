import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import SeekWake from './SeekWake.svelte';

describe('SeekWake', () => {
  it('is decorative: hidden from assistive technology, no text, nothing focusable', () => {
    const { container } = render(SeekWake, { songKey: 'a' });
    const wake = container.querySelector('.seek-wake')!;
    expect(wake.getAttribute('aria-hidden')).toBe('true');
    expect(wake.textContent).toBe('');
    expect(wake.querySelector('button, input, [tabindex]')).toBeNull();
  });

  it('is drawn again for another song (the key replays its CSS animation) and not for the same one', async () => {
    const { container, rerender } = render(SeekWake, { songKey: 'a' });
    const first = container.querySelector('.seek-wake');
    await rerender({ songKey: 'a' });
    expect(container.querySelector('.seek-wake')).toBe(first);
    await rerender({ songKey: 'b' });
    const second = container.querySelector('.seek-wake');
    expect(second).not.toBe(first);
    expect(container.querySelectorAll('.seek-wake')).toHaveLength(1);
  });
});
