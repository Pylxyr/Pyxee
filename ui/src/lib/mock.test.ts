import { describe, expect, it } from 'vitest';
import { DEMO_TRACKS, MockBackend } from './mock';

async function started(count = 3, start = 0) {
  const b = new MockBackend();
  await b.dispatch({ type: 'playTracks', tracks: DEMO_TRACKS.slice(0, count), start });
  return b;
}

describe('MockBackend', () => {
  it('goes loading -> playing only after the load time has passed', async () => {
    const b = await started();
    let s = await b.getState();
    expect(s.transport.state).toBe('loading');
    expect(s.audible).toBeNull();
    b.tick(250);
    s = await b.getState();
    expect(s.transport.state).toBe('loading');
    b.tick(300);
    s = await b.getState();
    expect(s.transport.state).toBe('playing');
    expect(s.audible).toBe(s.current);
  });

  it('advances gapless when a track ends and goes idle after the last one', async () => {
    const b = await started(2);
    b.tick(600);
    b.tick(DEMO_TRACKS[0]!.durationMs!);
    let s = await b.getState();
    expect(s.transport).toMatchObject({ state: 'playing' });
    expect(s.queue.find((e) => e.id === s.audible)?.track.key).toBe('demo:1');
    expect(s.positionMs).toBe(0);
    b.tick(DEMO_TRACKS[1]!.durationMs!);
    s = await b.getState();
    expect(s.transport.state).toBe('idle');
  });

  it('repeat one replays the same track', async () => {
    const b = await started(2);
    await b.dispatch({ type: 'setRepeat', repeat: 'one' });
    b.tick(600);
    const first = (await b.getState()).audible;
    b.tick(DEMO_TRACKS[0]!.durationMs!);
    const s = await b.getState();
    expect(s.audible).toBe(first);
    expect(s.positionMs).toBe(0);
  });

  it('previous restarts after three seconds, otherwise goes back', async () => {
    const b = await started(3, 1);
    b.tick(600);
    b.tick(5000);
    await b.dispatch({ type: 'previous' });
    let s = await b.getState();
    expect(s.positionMs).toBe(0);
    expect(s.current).toBe(s.queue[1]!.id);
    await b.dispatch({ type: 'previous' });
    s = await b.getState();
    expect(s.current).toBe(s.queue[0]!.id);
  });

  it('removing a queue entry can be undone in place', async () => {
    const b = await started(3);
    const ids = (await b.getState()).queue.map((e) => e.id);
    await b.dispatch({ type: 'remove', id: ids[2]! });
    let s = await b.getState();
    expect(s.queue).toHaveLength(2);
    expect(s.canUndo).toBe(true);
    await b.dispatch({ type: 'undo' });
    s = await b.getState();
    expect(s.queue.map((e) => e.id)).toEqual(ids);
    expect(s.canUndo).toBe(false);
  });

  it('removing the playing track moves on to the next one', async () => {
    const b = await started(3);
    b.tick(600);
    const s0 = await b.getState();
    await b.dispatch({ type: 'remove', id: s0.audible! });
    const s = await b.getState();
    expect(s.current).toBe(s.queue[0]!.id);
    expect(s.transport.state).toBe('loading');
  });

  it('pushes state to subscribers, and unsubscribing stops it', async () => {
    const b = new MockBackend();
    const seen: string[] = [];
    const off = await b.onState((s) => seen.push(s.transport.state));
    await b.dispatch({ type: 'playTracks', tracks: DEMO_TRACKS.slice(0, 1), start: 0 });
    off();
    await b.dispatch({ type: 'pause' });
    expect(seen).toEqual(['loading']);
  });
});
