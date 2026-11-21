import { coverColors, knownCoverColors, type CoverColors } from './color';
import { library } from './library.svelte';

/**
 * The colours of a cover, as it is read (a few milliseconds after the picture is there) and remembered. Call it while a
 * component is being set up. What was known stays until the next cover's colours arrive, so a change of song moves from
 * one colour to the next instead of flashing; nothing is read for a cover that has none.
 */
export function coverColorsOf(url: () => string | null): { readonly value: CoverColors | null } {
  const first = url();
  let value = $state.raw<CoverColors | null>(first ? (knownCoverColors(first) ?? null) : null);
  $effect(() => {
    const u = url();
    if (!u) {
      value = null;
      return;
    }
    const known = knownCoverColors(u);
    if (known) {
      value = known;
      return;
    }
    let stale = false;
    const b = library.backend;
    void coverColors(u, b ? (x) => b.coverBytes(x) : undefined).then((c) => {
      if (!stale && c) value = c;
    });
    return () => {
      stale = true;
    };
  });
  return {
    get value() {
      return value;
    },
  };
}
