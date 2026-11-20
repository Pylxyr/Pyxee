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

describe('rows and cover tiles reveal themselves as they scroll into view', () => {
  // An <li> start tag may contain `=>` (an arrow function in an attribute): read through it.
  const liWithReveal = /<li\b(?:=>|[^>])*\buse:reveal\b/;

  it.each(['components/TrackRow.svelte', 'components/EditableRow.svelte', 'components/CoverGrid.svelte', 'components/PlaylistGrid.svelte'])(
    '%s puts the reveal on its list item',
    (file) => {
      expect(read(file)).toMatch(liWithReveal);
    },
  );

  it('the playlist tiles of the library and the artist and playlist rows of a search have it too', () => {
    expect(read('pages/Library.svelte')).toMatch(/<li use:reveal>\s*<button class="cover-tile"/);
    expect(read('pages/Search.svelte').match(/<li class="trow" use:reveal>/g)).toHaveLength(2);
  });

  it('every list item of those surfaces has it: there is no row that is already there while the others arrive', () => {
    for (const file of ['components/TrackRow.svelte', 'components/EditableRow.svelte', 'components/CoverGrid.svelte', 'components/PlaylistGrid.svelte']) {
      const items = read(file).match(/<li\b(?:=>|[^>])*>/g) ?? [];
      expect(items.length, file).toBeGreaterThan(0);
      for (const item of items) expect(item, file).toMatch(/\buse:reveal\b/);
    }
  });

  it('Thumb marks its cover, the picture box or the placeholder, so the reveal can pop it', () => {
    expect(read('components/Thumb.svelte').match(/\bdata-cover\b/g)).toHaveLength(2);
  });

  it('the CSS hides what has not been revealed yet and keeps no per-position stagger', () => {
    const css = read('app.css');
    expect(css).toMatch(/\[data-reveal\] \{\s*opacity: 0;\s*\}/);
    // the old stagger (the first rows by their place in the list) and its keyframes are gone
    expect(css).not.toMatch(/\.route-in \.(trow|row-edit)/);
    expect(css).not.toMatch(/@keyframes row-in\b/);
    expect(css).not.toMatch(/animation-delay/);
  });
});

describe('search results', () => {
  const src = read('pages/Search.svelte');
  it('keep one results container while another filter loads, so the dimming is smooth', () => {
    expect(src).toMatch(/<div class="results route-in" data-dim=\{search\.status === 'loading'\}>/);
    // the container is kept while another filter loads: it shows whenever results are ready or reloading
    expect(src).toMatch(/showList = \$derived\(search\.status === 'ready' \|\| \(search\.status === 'loading' && search\.reloading\)\)/);
  });
});
