import { describe, expect, it } from 'vitest';
import { createRawSnippet } from 'svelte';
import { fireEvent, render } from '@testing-library/svelte';
import ScrollPage from './ScrollPage.svelte';

const snip = (html: string) => createRawSnippet(() => ({ render: () => html }));

describe('ScrollPage', () => {
  it('shows the edge under the header only once the body has scrolled past a few pixels, and hides it again at the top', async () => {
    const { getByTestId, container } = render(ScrollPage, { header: snip('<h1>Title</h1>'), children: snip('<p>Body</p>') });
    const section = container.querySelector('section')!;
    const body = getByTestId('page-scroll');
    expect(section.dataset.scrolled).toBe('false');
    body.scrollTop = 4;
    await fireEvent.scroll(body);
    expect(section.dataset.scrolled).toBe('false');
    body.scrollTop = 40;
    await fireEvent.scroll(body);
    expect(section.dataset.scrolled).toBe('true');
    body.scrollTop = 0;
    await fireEvent.scroll(body);
    expect(section.dataset.scrolled).toBe('false');
  });

  it('keeps the header, the focusable scroller and the scroll marker the app relies on', () => {
    const { getByTestId } = render(ScrollPage, { header: snip('<h1>T</h1>'), children: snip('x') });
    expect(getByTestId('page-header').classList.contains('page-edge')).toBe(true);
    expect(getByTestId('page-scroll').getAttribute('tabindex')).toBe('0');
    expect(getByTestId('page-scroll').hasAttribute('data-scroll')).toBe(true);
  });
});
