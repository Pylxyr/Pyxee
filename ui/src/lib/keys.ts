/**
 * Keys for a keyed `{#each}` over songs. A song can be in a list twice (history, a playlist), so the song's own key
 * alone is not unique: the occurrence count is added (`key`, `key#1`, `key#2`, ...). A row then keeps its identity when
 * rows before it come and go, so its cover does not lag and the row is not rebuilt.
 */
export function withKeys<T>(items: readonly T[], keyOf: (item: T) => string): { key: string; item: T; index: number }[] {
  const seen = new Map<string, number>();
  return items.map((item, index) => {
    const base = keyOf(item);
    const n = seen.get(base) ?? 0;
    seen.set(base, n + 1);
    return { key: n === 0 ? base : `${base}#${n}`, item, index };
  });
}
