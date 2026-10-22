import { describe, expect, it, vi } from 'vitest';
import type { Backend } from './backend';
import type { CommandDto } from './bindings/CommandDto';
import type { PlayerStateDto } from './bindings/PlayerStateDto';
import { DEMO_TRACKS, MockBackend } from './mock';
import { EMPTY_STATE, PlayerStore, describeFailure } from './player.svelte';
import { noDiagnostics, noLibrary, noShell } from './test-utils';

/** A backend that records commands and lets the test push state, like Rust does. */
function stub(initial: PlayerStateDto = EMPTY_STATE) {
  const sent: CommandDto[] = [];
  let push: (s: PlayerStateDto) => void = () => {};
  const backend: Backend = {
    ...noLibrary,
    ...noDiagnostics,
    ...noShell,
    getState: async () => initial,
    dispatch: async (c) => void sent.push(c),
    onState: async (cb) => ((push = cb), () => {}),
    windowInfo: async () => ({ backdrop: 'none' }),
    setVolume: async () => {},
    devCatalog: async () => DEMO_TRACKS,
    search: async () => ({ items: [] }),
    suggest: async () => [],
    searchFilters: async () => [],
  };
  return { backend, sent, push: (s: PlayerStateDto) => push(s) };
}

const entry = (id: number, key = `k${id}`) => ({
  id, origin: 'user' as const,
  track: { key, title: `Title ${id}`, artist: 'Artist', durationMs: 30_000, thumbnail: null },
});

/** Pretends the window is hidden (tray) or shown, and tells the page, like the browser does. */
function setHidden(hidden: boolean) {
  Object.defineProperty(document, 'hidden', { configurable: true, get: () => hidden });
  document.dispatchEvent(new Event('visibilitychange'));
}

describe('PlayerStore', () => {
  it('takes the backend state again when the page wakes up, and not while it is hidden', async () => {
    let current: PlayerStateDto = EMPTY_STATE;
    let reads = 0;
    const backend: Backend = {
      ...stub().backend,
      getState: async () => (reads++, current),
    };
    const p = new PlayerStore();
    await p.init(backend);
    expect(reads).toBe(1);
    current = { ...EMPTY_STATE, shuffle: true };
    setHidden(true);
    await Promise.resolve();
    expect(reads, 'hiding asks nothing').toBe(1);
    expect(p.state.shuffle).toBe(false);
    setHidden(false);
    await vi.waitFor(() => expect(p.state.shuffle).toBe(true));
    expect(reads).toBe(2);
  });

  it('keeps what it shows when the state cannot be read on waking', async () => {
    let fail = false;
    const backend: Backend = {
      ...stub().backend,
      getState: async () => {
        if (fail) throw new Error('not now');
        return { ...EMPTY_STATE, shuffle: true };
      },
    };
    const p = new PlayerStore();
    await p.init(backend);
    fail = true;
    await p.resync();
    expect(p.state.shuffle).toBe(true);
  });

  it('loads state and the demo catalog on init', async () => {
    const p = new PlayerStore();
    expect(p.ready).toBe(false);
    await p.init(new MockBackend());
    expect(p.ready).toBe(true);
    expect(p.catalog).toHaveLength(DEMO_TRACKS.length);
    expect(p.shown).toBeNull();
  });

  it('shows the loading track until the audible one takes over', async () => {
    const mock = new MockBackend();
    const p = new PlayerStore();
    await p.init(mock);
    await p.playTracks(DEMO_TRACKS.slice(0, 2));
    expect(p.shown?.track.key).toBe('demo:0');
    expect(p.state.audible).toBeNull();
    expect(p.isBusy).toBe(true);
    expect(p.isPlaying).toBe(false);
    mock.tick(600);
    expect(p.isPlaying).toBe(true);
    expect(p.state.audible).toBe(p.shown?.id);
  });

  it('never changes state locally: only backend pushes do', async () => {
    const s = stub();
    const p = new PlayerStore();
    await p.init(s.backend);
    await p.toggle();
    expect(s.sent).toEqual([{ type: 'toggle' }]);
    expect(p.state).toEqual(EMPTY_STATE);
  });

  it('keeps showing what is audible while the backend already knows about a later current track', async () => {
    const s = stub();
    const p = new PlayerStore();
    await p.init(s.backend);
    s.push({ ...EMPTY_STATE, queue: [entry(1), entry(2)], current: 2, audible: 1, upcoming: [2], transport: { state: 'playing', id: 1 } });
    expect(p.shown?.id).toBe(1);
    expect(p.upNext.map((e) => e.id)).toEqual([2]);
  });

  it('exposes failures with a human message', async () => {
    const s = stub();
    const p = new PlayerStore();
    await p.init(s.backend);
    s.push({ ...EMPTY_STATE, queue: [entry(1)], current: 1, transport: { state: 'failed', id: 1, failure: { kind: 'offline' } } });
    expect(p.failure).toEqual({ kind: 'offline' });
    expect(describeFailure(p.failure!)).toMatch(/offline/i);
    await p.retry();
    expect(s.sent.at(-1)).toEqual({ type: 'retry' });
  });

  it('cycles repeat off -> all -> one -> off', async () => {
    const s = stub();
    const p = new PlayerStore();
    await p.init(s.backend);
    for (const repeat of ['off', 'all', 'one'] as const) {
      s.push({ ...EMPTY_STATE, repeat });
      await p.cycleRepeat();
    }
    expect(s.sent).toEqual([
      { type: 'setRepeat', repeat: 'all' },
      { type: 'setRepeat', repeat: 'one' },
      { type: 'setRepeat', repeat: 'off' },
    ]);
  });

  it('rounds and clamps seek positions', async () => {
    const s = stub();
    const p = new PlayerStore();
    await p.init(s.backend);
    await p.seek(1234.6);
    await p.seek(-50);
    expect(s.sent).toEqual([{ type: 'seek', ms: 1235 }, { type: 'seek', ms: 0 }]);
  });
});

describe('describeFailure', () => {
  it('has a message for every failure kind', () => {
    for (const f of [
      { kind: 'offline' }, { kind: 'rateLimited' }, { kind: 'unavailable' },
      { kind: 'decode' }, { kind: 'expired', status: 403 }, { kind: 'expired', status: null },
      { kind: 'other', message: 'boom' },
    ] as const) {
      expect(describeFailure(f).length).toBeGreaterThan(3);
    }
    expect(describeFailure({ kind: 'other', message: 'boom' })).toBe('boom');
    expect(describeFailure({ kind: 'expired', status: 403 })).toBe('YouTube refused to send it (HTTP 403).');
    expect(describeFailure({ kind: 'expired', status: null })).toBe('YouTube refused to send it.');
  });
});
