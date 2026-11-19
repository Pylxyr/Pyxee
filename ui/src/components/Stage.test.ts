import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// The deck is wired to the player singleton, so what is checked here is the markup contract the CSS relies on.
describe('the deck cover marks whether the music is playing', () => {
  const src = readFileSync(resolve(process.cwd(), 'src/components/Stage.svelte'), 'utf8');
  it('sets data-paused from the transport state on the cover button (the glow layer reads it)', () => {
    expect(src).toMatch(/class="deck-art"[^>]*data-paused=\{!player\.isPlaying\}/);
  });
});
