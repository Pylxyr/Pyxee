import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const read = (p: string) => readFileSync(resolve(process.cwd(), 'src', p), 'utf8');

// These pages are wired to the player and search singletons; what is checked here is the markup contract the CSS relies on.
describe('a failed song shows its message with the nudge, on every surface', () => {
  it.each(['components/Stage.svelte', 'components/Dock.svelte', 'components/NowPlaying.svelte'])('%s', (file) => {
    expect(read(file)).toMatch(/class="[^"]*\bfail-in\b[^"]*"[^>]*role="alert"/);
  });
});

describe('search results arrive with the staggered rows only when a new set comes in', () => {
  const src = read('pages/Search.svelte');
  it('puts route-in on the results container, which a filter change keeps (so a replaced list does not animate)', () => {
    expect(src).toMatch(/<div class="results route-in" data-dim=\{search\.status === 'loading'\}>/);
    // the container is kept while another filter loads: it shows whenever results are ready or reloading
    expect(src).toMatch(/showList = \$derived\(search\.status === 'ready' \|\| \(search\.status === 'loading' && search\.reloading\)\)/);
  });
  it('the stagger it reuses is capped at eight rows, ending by 300 ms', () => {
    const css = read('app.css');
    expect(css).toMatch(/\.route-in \.trow:nth-child\(-n \+ 8\)/);
    expect(css).not.toMatch(/\.route-in \.trow:nth-child\(9\)/);
  });
});
