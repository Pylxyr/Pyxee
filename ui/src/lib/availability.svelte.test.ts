import { flushSync } from 'svelte';
import { describe, expect, it } from 'vitest';
import { AvailabilityStore } from './availability.svelte';

describe('AvailabilityStore.reset called from an effect', () => {
  it('runs that effect once per change of what it watches, not in a loop (On this device resets after every scan)', () => {
    const store = new AvailabilityStore();
    let revision = $state(0);
    let runs = 0;
    const stop = $effect.root(() => {
      $effect(() => {
        void revision;
        runs++;
        store.reset();
      });
    });
    flushSync();
    expect(runs).toBe(1);
    expect(store.epoch).toBe(1);
    revision = 1;
    flushSync();
    expect(runs).toBe(2);
    expect(store.epoch).toBe(2);
    stop();
  });

  it('still lets rows that read the epoch ask again after a reset', () => {
    const store = new AvailabilityStore();
    let seen = 0;
    const stop = $effect.root(() => {
      $effect(() => {
        void store.epoch;
        seen++;
      });
    });
    flushSync();
    store.reset();
    flushSync();
    expect(seen).toBe(2);
    stop();
  });
});
