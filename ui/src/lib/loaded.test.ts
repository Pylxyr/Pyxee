import { describe, expect, it } from 'vitest';
import { Loaded } from './loaded.svelte';

const later = <T>() => {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => ((resolve = res), (reject = rej)));
  return { promise, resolve, reject };
};

describe('Loaded', () => {
  it('is not loaded until the first answer, then holds it', async () => {
    const l = new Loaded<number>();
    const p = l.load(async () => 1);
    expect(l.loaded).toBe(false);
    expect(l.loading).toBe(true);
    await p;
    expect([l.data, l.loaded, l.loading]).toEqual([1, true, false]);
  });

  it('an older answer that arrives late never replaces a newer one', async () => {
    const l = new Loaded<string>();
    const slow = later<string>();
    const first = l.load(() => slow.promise);
    await l.load(async () => 'new');
    slow.resolve('old');
    await first;
    expect(l.data).toBe('new');
  });

  it('a failed read keeps the data and says why', async () => {
    const l = new Loaded<string>();
    await l.load(async () => 'kept');
    await l.load(() => Promise.reject('The network is down.'));
    expect([l.data, l.error, l.missing]).toEqual(['kept', 'The network is down.', false]);
    await l.load(async () => 'back');
    expect([l.data, l.error]).toEqual(['back', '']);
  });

  it('only a not-found answer means the thing is gone', async () => {
    const l = new Loaded<string>();
    await l.load(() => Promise.reject('That playlist no longer exists.'));
    expect([l.data, l.missing, l.loaded, l.error]).toEqual([null, true, true, '']);
  });

  it('reset forgets everything and drops a read still on its way', async () => {
    const l = new Loaded<string>();
    const slow = later<string>();
    const p = l.load(() => slow.promise);
    l.reset();
    slow.resolve('stale');
    await p;
    expect([l.data, l.loaded, l.loading]).toEqual([null, false, false]);
  });
});
