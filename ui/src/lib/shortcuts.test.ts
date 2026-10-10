import { describe, expect, it } from 'vitest';
import { actionForKey, type KeyContext } from './shortcuts';

const ctx: KeyContext = { positionMs: 10_000, durationMs: 60_000, nowPlayingOpen: false, hasTrack: true };
const cmd = (key: string, target?: EventTarget, c = ctx) => actionForKey({ key, target }, c);

describe('actionForKey', () => {
  it('space toggles playback', () => {
    expect(cmd(' ')).toEqual({ kind: 'command', command: { type: 'toggle' } });
  });
  it('arrows seek by five seconds, clamped to the track', () => {
    expect(cmd('ArrowRight')).toEqual({ kind: 'command', command: { type: 'seek', ms: 15_000 } });
    expect(cmd('ArrowLeft', undefined, { ...ctx, positionMs: 2000 })).toEqual({ kind: 'command', command: { type: 'seek', ms: 0 } });
    expect(cmd('ArrowRight', undefined, { ...ctx, positionMs: 58_000 })).toEqual({ kind: 'command', command: { type: 'seek', ms: 60_000 } });
  });
  it('n and p skip', () => {
    expect(cmd('n')).toEqual({ kind: 'command', command: { type: 'next' } });
    expect(cmd('P')).toEqual({ kind: 'command', command: { type: 'previous' } });
  });
  it('does nothing without a track', () => {
    expect(cmd(' ', undefined, { ...ctx, hasTrack: false })).toBeNull();
  });
  it('never steals keys while typing in a text field', () => {
    const input = document.createElement('input');
    input.type = 'search';
    for (const k of [' ', 'n', 'p', 'ArrowLeft']) expect(cmd(k, input)).toBeNull();
  });
  it('leaves space and arrows to buttons and sliders', () => {
    const button = document.createElement('button');
    const range = document.createElement('input');
    range.type = 'range';
    expect(cmd(' ', button)).toBeNull();
    expect(cmd('ArrowRight', range)).toBeNull();
    expect(cmd('n', button)).not.toBeNull();
  });
  it('ignores shortcuts combined with modifiers', () => {
    expect(actionForKey({ key: 'n', ctrlKey: true }, ctx)).toBeNull();
  });
  it('escape closes Now Playing only when it is open', () => {
    expect(cmd('Escape')).toBeNull();
    expect(cmd('Escape', undefined, { ...ctx, nowPlayingOpen: true })).toEqual({ kind: 'close' });
  });
});
