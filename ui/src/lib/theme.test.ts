import { describe, expect, it } from 'vitest';
import { DEFAULT_ACCENT, ThemeStore, resolveTheme } from './theme.svelte';

function memory(initial: Record<string, string> = {}) {
  const data = { ...initial };
  return { data, getItem: (k: string) => data[k] ?? null, setItem: (k: string, v: string) => void (data[k] = v) };
}
function fakeMq(matches: boolean) {
  let listener: ((e: { matches: boolean }) => void) | null = null;
  return {
    mq: { matches, addEventListener: (_: string, cb: typeof listener) => (listener = cb) } as unknown as MediaQueryList,
    flip: (m: boolean) => listener?.({ matches: m }),
  };
}

describe('resolveTheme', () => {
  it('follows the system only in system mode', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
    expect(resolveTheme('light', true)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
  });
});

describe('ThemeStore', () => {
  it('defaults to system mode with the red accent', () => {
    const t = new ThemeStore(memory(), fakeMq(true).mq);
    expect(t.settings).toEqual({ mode: 'system', accent: DEFAULT_ACCENT, dynamicColor: false });
    expect(t.resolved).toBe('dark');
  });
  it('reacts to the system dark/light setting changing', () => {
    const { mq, flip } = fakeMq(true);
    const t = new ThemeStore(memory(), mq);
    flip(false);
    expect(t.resolved).toBe('light');
  });
  it('persists changes and restores them', () => {
    const store = memory();
    new ThemeStore(store, null).update({ mode: 'light', dynamicColor: true });
    const again = new ThemeStore(store, null);
    expect(again.settings.mode).toBe('light');
    expect(again.settings.dynamicColor).toBe(true);
  });
  it('ignores corrupt stored settings', () => {
    const t = new ThemeStore(memory({ 'pyxee.theme': '{"mode":"neon","accent":"red"}' }), null);
    expect(t.settings.mode).toBe('system');
    expect(t.settings.accent).toBe(DEFAULT_ACCENT);
    expect(new ThemeStore(memory({ 'pyxee.theme': 'not json' }), null).settings.mode).toBe('system');
  });
  it('uses the art colour only when dynamic colour is on, and keeps it readable', () => {
    const t = new ThemeStore(memory(), fakeMq(true).mq);
    t.art = '#101030';
    expect(t.accent).toBe(DEFAULT_ACCENT);
    t.update({ dynamicColor: true });
    expect(t.accent).not.toBe('#101030'); // lightened for contrast on the dark theme
    expect(t.accent).not.toBe(DEFAULT_ACCENT);
    t.art = null;
    expect(t.accent).toBe(DEFAULT_ACCENT);
  });
  it('writes theme, backdrop and accent to the root element', () => {
    const t = new ThemeStore(memory(), fakeMq(false).mq);
    t.backdrop = 'mica';
    const root = document.createElement('html');
    t.apply(root);
    expect(root.dataset.theme).toBe('light');
    expect(root.dataset.backdrop).toBe('mica');
    expect(root.style.getPropertyValue('--accent')).toBe(DEFAULT_ACCENT);
  });
});
