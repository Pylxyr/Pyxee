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
  it('F11 switches full screen, with or without a track', () => {
    expect(actionForKey({ key: 'F11' }, ctx)).toEqual({ kind: 'fullscreen' });
    expect(actionForKey({ key: 'F11' }, { ...ctx, hasTrack: false })).toEqual({ kind: 'fullscreen' });
  });
  it('escape closes Now Playing only when it is open', () => {
    expect(cmd('Escape')).toBeNull();
    expect(cmd('Escape', undefined, { ...ctx, nowPlayingOpen: true })).toEqual({ kind: 'close' });
  });
});

describe('Space and arrows with a focused control', () => {
  const el = (html: string) => {
    document.body.innerHTML = html;
    return document.body.firstElementChild as HTMLElement;
  };

  it('a focused button keeps Space (that is how it is pressed), but arrows still seek', () => {
    const b = el('<button>Shuffle</button>');
    expect(cmd(' ', b)).toBeNull();
    expect(cmd('ArrowRight', b)).toEqual({ kind: 'command', command: { type: 'seek', ms: 15_000 } });
  });

  it('after a mouse click on a song row, Space is play/pause', () => {
    const b = el('<ul><li class="trow"><button class="trow-main">Song</button></li></ul>').querySelector('button')!;
    expect(cmd(' ', b)).toBeNull();
    expect(cmd(' ', b, { ...ctx, pointerFocus: true })).toEqual({ kind: 'command', command: { type: 'toggle' } });
    // a row reached by Tab keeps Space as a press
    expect(cmd(' ', b, { ...ctx, pointerFocus: false })).toBeNull();
  });

  it('arrows are left to sliders, selects, tabs and menus', () => {
    for (const html of ['<input type="range">', '<select></select>', '<button role="tab">T</button>', '<div role="tablist"><button>x</button></div>', '<input type="radio">']) {
      const t = el(html);
      expect(cmd('ArrowLeft', t.querySelector('button') ?? t), html).toBeNull();
    }
  });

  it('Space belongs to a checkbox, a select and a switch', () => {
    for (const html of ['<input type="checkbox">', '<select></select>', '<div role="switch" tabindex="0"></div>']) expect(cmd(' ', el(html)), html).toBeNull();
  });
});

describe('search shortcuts', () => {
  it('/ and Ctrl+K focus the search box, but / is typed text inside a field', () => {
    expect(cmd('/')).toEqual({ kind: 'search' });
    expect(actionForKey({ key: 'k', ctrlKey: true }, ctx)).toEqual({ kind: 'search' });
    expect(actionForKey({ key: 'K', metaKey: true }, ctx)).toEqual({ kind: 'search' });
    expect(cmd('/', document.createElement('input'))).toBeNull();
    expect(actionForKey({ key: 'k', ctrlKey: true, shiftKey: true }, ctx)).toBeNull();
  });
  it('they work with nothing playing, and not behind Now Playing', () => {
    expect(cmd('/', undefined, { ...ctx, hasTrack: false })).toEqual({ kind: 'search' });
    expect(cmd('/', undefined, { ...ctx, nowPlayingOpen: true })).toBeNull();
  });
});
