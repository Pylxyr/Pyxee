import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import SectionHead from './SectionHead.svelte';

/** The heading is drawn right before a `.cover-grid` inside `.shelves`, which is what gives it the paging buttons. */
function mount() {
  const shelves = document.createElement('div');
  shelves.className = 'shelves';
  const grid = document.createElement('div');
  grid.className = 'cover-grid';
  shelves.append(grid);
  document.body.append(shelves);
  const children = createRawSnippet(() => ({ render: () => '<span>Title</span>' }));
  return render(SectionHead, { target: shelves, anchor: grid, props: { children } });
}

beforeEach(() => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} unobserve() {} });
});

describe('shelf paging buttons', () => {
  // Both buttons draw the "down" chevron turned on its side. A positive CSS rotation is clockwise, which turns a
  // down chevron to point left; a negative one turns it to point right.
  it('the back button points left and the forward button points right', () => {
    const { getByLabelText } = mount();
    const back = getByLabelText('Scroll back');
    const forward = getByLabelText('Scroll forward');
    expect(back.classList.contains('rotate-90')).toBe(true);
    expect(back.classList.contains('-rotate-90')).toBe(false);
    expect(forward.classList.contains('-rotate-90')).toBe(true);
    expect(forward.classList.contains('rotate-90')).toBe(false);
  });
});
