import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Mirror, PREFIX, resetPersist, restore, restoreSettings, snapshot, persist, type Items, type PrefsFile, type Reader } from './persist';

class Memory implements Reader {
  #map = new Map<string, string>();
  get length() {
    return this.#map.size;
  }
  key(i: number) {
    return [...this.#map.keys()][i] ?? null;
  }
  getItem(k: string) {
    return this.#map.get(k) ?? null;
  }
  setItem(k: string, v: string) {
    this.#map.set(k, v);
  }
  removeItem(k: string) {
    this.#map.delete(k);
  }
}

function fileOf(initial: Items | Error = {}) {
  let held: Items = initial instanceof Error ? {} : initial;
  const sets: Items[] = [];
  const file: PrefsFile = {
    get: async () => {
      if (initial instanceof Error) throw initial;
      return held;
    },
    set: async (items) => {
      sets.push(items);
      held = items;
    },
  };
  return { file, sets, held: () => held };
}

describe('snapshot', () => {
  it('copies only the page entries, in a fixed order', () => {
    const s = new Memory();
    s.setItem('pyxee.volume', '0.4');
    s.setItem('other', 'x');
    s.setItem('pyxee.theme', '{}');
    expect(Object.keys(snapshot(s))).toEqual(['pyxee.theme', 'pyxee.volume']);
  });
});

describe('restore', () => {
  it('fills in what the web view lost and never replaces what it has', async () => {
    const s = new Memory();
    s.setItem('pyxee.volume', '0.9');
    const { file } = fileOf({ 'pyxee.volume': '0.1', 'pyxee.theme': '{"id":"ink"}', foreign: 'x' });
    const { restored } = await restore(s, file);
    expect(restored).toEqual(['pyxee.theme']);
    expect(s.getItem('pyxee.volume')).toBe('0.9');
    expect(s.getItem('pyxee.theme')).toBe('{"id":"ink"}');
    expect(s.getItem('foreign')).toBeNull();
  });

  it('goes on with what the web view has when the app cannot answer', async () => {
    const s = new Memory();
    expect(await restore(s, fileOf(new Error('no')).file)).toEqual({ saved: null, restored: [] });
    const never: PrefsFile = { get: () => new Promise(() => {}), set: async () => {} };
    expect((await restore(s, never, 5)).saved).toBeNull();
  });
});

describe('Mirror', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('sends a burst of changes once, after a quiet moment', async () => {
    const s = new Memory();
    const { file, sets } = fileOf();
    const m = new Mirror(s, file, {}, 100);
    s.setItem('pyxee.volume', '0.2');
    m.touch();
    s.setItem('pyxee.volume', '0.3');
    m.touch();
    await vi.advanceTimersByTimeAsync(99);
    expect(sets).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(2);
    expect(sets).toEqual([{ 'pyxee.volume': '0.3' }]);
  });

  it('sends nothing when the file already holds the same set', async () => {
    const s = new Memory();
    s.setItem('pyxee.volume', '0.2');
    const { file, sets } = fileOf();
    await new Mirror(s, file, { 'pyxee.volume': '0.2' }, 10).flush();
    expect(sets).toHaveLength(0);
  });

  it('sends the first time when the file could not be read, and tries again after a failed send', async () => {
    const s = new Memory();
    s.setItem('pyxee.volume', '0.2');
    let fail = true;
    const sent: Items[] = [];
    const file: PrefsFile = {
      get: async () => ({}),
      set: async (i) => {
        if (fail) throw new Error('disk');
        sent.push(i);
      },
    };
    const m = new Mirror(s, file, null, 10);
    await m.flush();
    expect(sent).toHaveLength(0);
    fail = false;
    await m.flush();
    expect(sent).toEqual([{ 'pyxee.volume': '0.2' }]);
  });

  it('sends a removed entry too', async () => {
    const s = new Memory();
    s.setItem('pyxee.look', '1');
    const { file, sets } = fileOf({ 'pyxee.look': '1' });
    const m = new Mirror(s, file, { 'pyxee.look': '1' }, 10);
    s.removeItem('pyxee.look');
    await m.flush();
    expect(sets).toEqual([{}]);
  });
});

describe('restoreSettings', () => {
  afterEach(() => resetPersist());

  it('does nothing without a file (a browser, the tests)', async () => {
    await expect(restoreSettings({ mirror: true, storage: new Memory() })).resolves.toBeUndefined();
    expect(() => persist.touch()).not.toThrow();
  });

  it('restores, then copies the settings the file has never seen', async () => {
    const s = new Memory();
    s.setItem('pyxee.crossfade', '4');
    const { file, held } = fileOf({ 'pyxee.theme': '{"id":"ink"}' });
    await restoreSettings({ mirror: true, storage: s, file });
    await Promise.resolve();
    await Promise.resolve();
    expect(s.getItem('pyxee.theme')).toBe('{"id":"ink"}');
    expect(held()).toEqual({ 'pyxee.crossfade': '4', 'pyxee.theme': '{"id":"ink"}' });
  });

  it('only reads in the small windows', async () => {
    const s = new Memory();
    s.setItem('pyxee.crossfade', '4');
    const { file, sets } = fileOf({});
    await restoreSettings({ mirror: false, storage: s, file });
    persist.touch();
    expect(sets).toHaveLength(0);
  });
});

describe('every setting the page saves is copied', () => {
  // A new place that writes `pyxee.*` to localStorage must call persist.touch() or its setting is lost in an update.
  const dir = __dirname;
  const files = ['look', 'prefs', 'seekmode', 'search', 'theme'].map((n) => `${n}.svelte.ts`);
  it.each(files)('%s touches after each write', (name) => {
    const text = readFileSync(join(dir, name), 'utf8');
    const writes = text.match(/\.(setItem|removeItem)\(/g)?.length ?? 0;
    const touches = text.match(/persist\.touch\(\)/g)?.length ?? 0;
    expect(writes).toBeGreaterThan(0);
    expect(touches).toBe(writes);
  });
  it('no other source file writes to localStorage', () => {
    const found: string[] = [];
    const walk = (d: string) => {
      for (const e of readdirSync(d, { withFileTypes: true })) {
        const p = join(d, e.name);
        if (e.isDirectory()) walk(p);
        else if (/\.(ts|svelte)$/.test(e.name) && !e.name.endsWith('.test.ts') && e.name !== 'persist.ts' && /(?:[sS]torage\??\.|#storage\??\.)(setItem|removeItem)\(/.test(readFileSync(p, 'utf8'))) found.push(e.name);
      }
    };
    walk(join(dir, '..'));
    expect(found.sort()).toEqual([...files].sort());
  });
  it('uses the prefix the app accepts', () => expect(PREFIX).toBe('pyxee.'));
});
