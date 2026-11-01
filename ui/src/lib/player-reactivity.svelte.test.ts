import { flushSync } from 'svelte';
import { describe, expect, it } from 'vitest';
import type { Backend } from './backend';
import type { PlayerStateDto } from './bindings/PlayerStateDto';
import type { PositionDto } from './bindings/PositionDto';
import { EMPTY_STATE, PlayerStore } from './player.svelte';

const entry = (id: number) => ({ id, origin: 'user' as const, track: { key: `k${id}`, title: `T${id}`, artist: 'A', durationMs: 200_000, thumbnail: null } });

describe('the 4 Hz position pushes', () => {
  it('re-run only what shows the position, not what reads the queue or the shown song', async () => {
    let pushState: (s: PlayerStateDto) => void = () => {};
    let pushPos: (p: PositionDto) => void = () => {};
    const backend = {
      onState: async (cb: (s: PlayerStateDto) => void) => ((pushState = cb), () => {}),
      onPosition: async (cb: (p: PositionDto) => void) => ((pushPos = cb), () => {}),
      getState: async () => EMPTY_STATE,
      devCatalog: async () => [],
    } as unknown as Backend;
    const store = new PlayerStore();
    await store.init(backend);
    pushState({
      ...EMPTY_STATE,
      seq: 2,
      transport: { state: 'playing', id: 1 },
      audible: 1,
      current: 1,
      queue: [entry(1), entry(2), entry(3)],
      upcoming: [2, 3],
    });

    const runs = { upNext: 0, shown: 0, playing: 0, position: 0 };
    const stop = $effect.root(() => {
      $effect(() => {
        void store.upNext;
        runs.upNext++;
      });
      $effect(() => {
        void store.shown;
        runs.shown++;
      });
      $effect(() => {
        void store.isPlaying;
        runs.playing++;
      });
      $effect(() => {
        void store.positionMs;
        runs.position++;
      });
    });
    flushSync();
    expect(runs).toEqual({ upNext: 1, shown: 1, playing: 1, position: 1 });

    const before = store.state;
    for (let i = 1; i <= 20; i++) {
      pushPos({ seq: 2 + i, positionMs: i * 250 });
      flushSync();
    }
    expect(store.positionMs).toBe(5000);
    expect(store.state).toBe(before); // the same object: nothing that reads it was invalidated
    expect(runs).toEqual({ upNext: 1, shown: 1, playing: 1, position: 21 });
    stop();
  });
});
