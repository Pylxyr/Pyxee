import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TrackDto } from './bindings/TrackDto';

const track = (n: number): TrackDto => ({ key: `t${n}`, title: `Song ${n}`, artist: 'A', durationMs: 1000, thumbnail: null });

const playTracks = vi.fn(async (..._args: unknown[]) => {});
const addToQueue = vi.fn(async (_t: TrackDto) => {});
const push = vi.fn();
const relatedGet = vi.fn();
const shown: { track: TrackDto | null } = { track: null };

vi.mock('./player.svelte', () => ({
  player: {
    playTracks: (...a: unknown[]) => playTracks(...a),
    addToQueue: (t: TrackDto) => addToQueue(t),
    get shown() {
      return shown.track ? { track: shown.track } : null;
    },
  },
}));
vi.mock('./library.svelte', () => ({ library: { get backend() { return { relatedGet }; } } }));
vi.mock('./snackbar.svelte', () => ({ snackbar: { push: (...a: unknown[]) => push(...a) } }));

import { playWithRelated, RADIO_SIZE } from './radio';

beforeEach(() => {
  playTracks.mockClear();
  addToQueue.mockClear();
  push.mockClear();
  relatedGet.mockReset();
  shown.track = null;
});

describe('picking a song starts a radio', () => {
  it('plays the song at once, then queues similar songs behind it, once each and never the song itself', async () => {
    relatedGet.mockResolvedValue([track(1), track(2), track(2), track(0), track(3)]);
    shown.track = track(0);
    await playWithRelated(track(0));
    expect(playTracks).toHaveBeenCalledWith([track(0)], 0);
    expect(relatedGet).toHaveBeenCalledWith('t0', RADIO_SIZE);
    expect(addToQueue.mock.calls.map((c) => c[0].key)).toEqual(['t1', 't2', 't3']);
  });

  it('keeps the song playing and says so when similar songs cannot be found', async () => {
    relatedGet.mockRejectedValue(new Error('offline'));
    await playWithRelated(track(0));
    expect(playTracks).toHaveBeenCalledTimes(1);
    expect(addToQueue).not.toHaveBeenCalled();
    expect(push).toHaveBeenCalledTimes(1);
  });

  it('leaves the queue alone once the person has moved on to another song', async () => {
    relatedGet.mockResolvedValue([track(1), track(2)]);
    shown.track = track(9);
    await playWithRelated(track(0));
    expect(addToQueue).not.toHaveBeenCalled();
  });
});
