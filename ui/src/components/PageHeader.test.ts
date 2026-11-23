import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import PageHeader from './PageHeader.svelte';

const children = createRawSnippet(() => ({ render: () => '<div class="chips">chips</div>' }));

describe('page header', () => {
  it('puts short tabs on the title line only when the page says they are short (inline)', () => {
    const inline = render(PageHeader, { props: { title: 'Library', inline: true, children } });
    expect(inline.container.querySelector('.page-head')!.hasAttribute('data-inline')).toBe(true);
    inline.unmount();
    // Chips that can need two lines of their own keep the full-width row under the title.
    const stacked = render(PageHeader, { props: { title: 'Explore', children } });
    expect(stacked.container.querySelector('.page-head')!.hasAttribute('data-inline')).toBe(false);
    expect(stacked.container.querySelector('.page-head-below')).toBeTruthy();
  });

  it('has nothing to put inline when there is no content below the title', () => {
    const { container } = render(PageHeader, { props: { title: 'Queue', inline: true } });
    expect(container.querySelector('.page-head')!.hasAttribute('data-inline')).toBe(false);
    expect(container.querySelector('.page-head-below')).toBeNull();
  });
});
