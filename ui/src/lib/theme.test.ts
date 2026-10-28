import { describe, expect, it } from 'vitest';
import { contrast, mixHex, paletteFromArt, windowTint } from './color';
import { ThemeStore } from './theme.svelte';
import { BUILTIN_THEMES, MAX_CUSTOM_THEMES, contrastChecks, type ThemeDef } from './themes';

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
const mine = (patch: Partial<ThemeDef> = {}): ThemeDef => ({ id: '', name: 'Mine', scheme: 'dark', colors: { ...BUILTIN_THEMES[0]!.colors, accent: '#00ff88' }, ...patch });

describe('ThemeStore', () => {
  it('follows the system by default: Dark when the system is dark, Light when it is light', () => {
    const { mq, flip } = fakeMq(true);
    const t = new ThemeStore(memory(), mq);
    expect(t.settings.followSystem).toBe(true);
    expect(t.active.id).toBe('dark');
    expect(t.windowTheme).toBeNull();
    flip(false);
    expect(t.active.id).toBe('light');
    expect(t.resolved).toBe('light');
  });

  it('uses the themes chosen for system light and system dark', () => {
    const { mq, flip } = fakeMq(true);
    const t = new ThemeStore(memory(), mq);
    t.update({ darkId: 'oled', lightId: 'sky' });
    expect(t.active.id).toBe('oled');
    flip(false);
    expect(t.active.id).toBe('sky');
  });

  it('picking a theme stops following the system and pins the native window to that theme', () => {
    const t = new ThemeStore(memory(), fakeMq(false).mq);
    t.select('deepblue');
    expect(t.settings.followSystem).toBe(false);
    expect(t.active.id).toBe('deepblue');
    expect(t.windowTheme).toBe('dark'); // dark theme on a light system: the frame must be dark too
    t.select('nonsense');
    expect(t.active.id).toBe('deepblue');
  });

  it('persists changes and restores them', () => {
    const store = memory();
    const a = new ThemeStore(store, null);
    a.select('oled');
    a.update({ dynamicColor: true });
    const saved = a.saveCustom(mine());
    const b = new ThemeStore(store, null);
    expect(b.settings.themeId).toBe('oled');
    expect(b.settings.dynamicColor).toBe(true);
    expect(b.find(saved.id)?.name).toBe('Mine');
  });

  it('understands settings saved before themes existed', () => {
    const dark = new ThemeStore(memory({ 'pyxee.theme': '{"mode":"dark","accent":"#ff3b4a","dynamicColor":true}' }), fakeMq(false).mq);
    expect(dark.active.id).toBe('dark');
    expect(dark.settings.followSystem).toBe(false);
    expect(dark.settings.dynamicColor).toBe(true);
    expect(new ThemeStore(memory({ 'pyxee.theme': '{"mode":"light"}' }), fakeMq(true).mq).active.id).toBe('light');
    expect(new ThemeStore(memory({ 'pyxee.theme': '{"mode":"system"}' }), fakeMq(true).mq).settings.followSystem).toBe(true);
  });

  it('ignores corrupt or hostile stored settings', () => {
    expect(new ThemeStore(memory({ 'pyxee.theme': 'not json' }), null).settings.followSystem).toBe(true);
    const bad = JSON.stringify({
      followSystem: false, themeId: 'gone', darkId: 'light', lightId: 'dark',
      custom: [{ id: 'dark', name: 'Takes over a built-in', scheme: 'dark', colors: BUILTIN_THEMES[0]!.colors }, { id: 'x', name: 'no colours', scheme: 'dark' }],
    });
    const t = new ThemeStore(memory({ 'pyxee.theme': bad }), null);
    expect(t.settings.custom).toEqual([]);
    expect(t.active.id).toBe('dark'); // unknown id falls back
    expect(t.settings.darkId).toBe('dark'); // a light theme cannot be the system-dark theme
    expect(t.settings.lightId).toBe('light');
  });

  it('makes the whole palette from the art colour only when dynamic colour is on, and it stays readable', () => {
    const t = new ThemeStore(memory(), fakeMq(true).mq);
    t.art = '#101030';
    expect(t.colors).toEqual(t.active.colors);
    t.update({ dynamicColor: true });
    expect(t.colors.bg).not.toBe(t.active.colors.bg);
    expect(t.accent).toBe(t.colors.accent);
    expect(contrastChecks(t.colors).filter((c) => !c.ok)).toEqual([]);
    t.select('sky'); // a light theme stays light: the palette is light too
    expect(t.colors.bg).not.toBe(t.active.colors.bg);
    expect(contrastChecks(t.colors).filter((c) => !c.ok)).toEqual([]);
    t.art = null;
    expect(t.colors).toEqual(t.active.colors);
  });

  it('a palette from any cover colour reads, in both schemes', () => {
    for (const art of ['#ff0000', '#00ff00', '#0000ff', '#fff200', '#808080', '#101030', '#f5e6c8', '#d9c9b0', '#e03060', '#00ffd0', '#7a2cff', '#ff7a00']) {
      for (const scheme of ['dark', 'light'] as const) {
        expect(contrastChecks(paletteFromArt(art, scheme)).filter((c) => !c.ok), `${art} ${scheme}`).toEqual([]);
      }
    }
  });

  it('a coloured cover tints the surfaces clearly: deep in dark mode, pale in light mode, grey stays grey', () => {
    const dark = paletteFromArt('#e03060', 'dark');
    const light = paletteFromArt('#e03060', 'light');
    expect(contrast(dark.bg, '#000000')).toBeGreaterThan(1.1);
    expect(contrast(dark.bg, '#000000')).toBeLessThan(3);
    expect(contrast(light.bg, '#ffffff')).toBeGreaterThan(1.05);
    expect(contrast(light.bg, '#ffffff')).toBeLessThan(1.5);
    expect(contrast(dark.fg, dark.surface2)).toBeGreaterThanOrEqual(7);
    expect(contrast(light.fg, light.surface2)).toBeGreaterThanOrEqual(7);
    const grey = paletteFromArt('#808080', 'dark');
    expect(grey.bg.slice(1, 3)).toBe(grey.bg.slice(3, 5));
    expect(grey.bg.slice(3, 5)).toBe(grey.bg.slice(5, 7));
  });

  it('the native window takes a border and title bar colour from the palette only while the cover colours the app', () => {
    const t = new ThemeStore(memory(), fakeMq(true).mq);
    t.art = '#e03060';
    expect(t.nativeTint).toBeNull();
    t.update({ dynamicColor: true });
    expect(t.nativeTint).toEqual(windowTint(t.colors));
    expect(t.nativeTint?.caption).toBe(t.colors.bg);
    expect(t.nativeTint?.text).toBe(t.colors.fg);
    expect(t.nativeTint?.border).not.toBe(t.colors.line);
    t.art = null;
    expect(t.nativeTint).toBeNull();
  });

  it('mixes two colours', () => {
    expect(mixHex('#000000', '#ffffff', 0)).toBe('#000000');
    expect(mixHex('#000000', '#ffffff', 1)).toBe('#ffffff');
    expect(mixHex('#000000', '#ffffff', 0.5)).toMatch(/^#(7f|80){3}$/);
  });

  it('writes the scheme, backdrop and every colour to the root element', () => {
    const t = new ThemeStore(memory(), fakeMq(false).mq);
    t.backdrop = 'mica';
    t.select('oled');
    const root = document.createElement('html');
    t.apply(root);
    expect(root.dataset.theme).toBe('dark');
    expect(root.dataset.themeId).toBe('oled');
    expect(root.dataset.backdrop).toBe('mica');
    expect(root.style.getPropertyValue('--t-bg')).toBe('#000000');
    expect(root.style.getPropertyValue('--t-accent')).toBe('#ff5c6a');
    expect(contrast(root.style.getPropertyValue('--t-on-accent'), '#ff5c6a')).toBeGreaterThanOrEqual(4.5);
  });

  it('text on the accent stays readable when the dynamic accent is light (the old dark-mode bug)', () => {
    const t = new ThemeStore(memory(), fakeMq(true).mq);
    t.update({ dynamicColor: true });
    t.art = '#101030'; // lightened for the dark theme
    const root = document.createElement('html');
    t.apply(root);
    const accent = root.style.getPropertyValue('--t-accent');
    expect(contrast(root.style.getPropertyValue('--t-on-accent'), accent)).toBeGreaterThanOrEqual(4.5);
  });

  describe('custom themes', () => {
    it('saves a new one with an id of its own and replaces it on the next save', () => {
      const t = new ThemeStore(memory(), null);
      const a = t.saveCustom(mine({ name: '  Night  ' }));
      expect(a.id).toMatch(/^custom-/);
      expect(a.name).toBe('Night');
      const b = t.saveCustom({ ...a, name: 'Night 2' });
      expect(b.id).toBe(a.id);
      expect(t.settings.custom).toHaveLength(1);
      expect(t.settings.custom[0]!.name).toBe('Night 2');
    });

    it('shows a theme being edited live without saving it', () => {
      const t = new ThemeStore(memory(), fakeMq(true).mq);
      t.preview = mine({ id: '', colors: { ...BUILTIN_THEMES[0]!.colors, bg: '#102030' } });
      expect(t.active.colors.bg).toBe('#102030');
      expect(t.settings.custom).toEqual([]);
      t.preview = null;
      expect(t.active.id).toBe('dark');
    });

    it('switching a theme from dark to light takes it out of the system-dark slot', () => {
      const t = new ThemeStore(memory(), null);
      const a = t.saveCustom(mine());
      t.update({ darkId: a.id });
      t.saveCustom({ ...a, scheme: 'light', colors: BUILTIN_THEMES[1]!.colors });
      expect(t.settings.darkId).toBe('dark');
    });

    it('deleting the selected theme falls back to a built-in one', () => {
      const t = new ThemeStore(memory(), fakeMq(true).mq);
      const a = t.saveCustom(mine());
      t.select(a.id);
      t.deleteCustom(a.id);
      expect(t.settings.themeId).toBe('dark');
      expect(t.settings.custom).toEqual([]);
      expect(t.active.id).toBe('dark');
    });

    it(`keeps at most ${MAX_CUSTOM_THEMES}`, () => {
      const t = new ThemeStore(memory(), null);
      for (let i = 0; i < MAX_CUSTOM_THEMES; i++) t.saveCustom(mine({ name: `n${i}` }));
      expect(() => t.saveCustom(mine())).toThrow(/up to 30/);
    });
  });
});

describe('theme shared between windows', () => {
  it('reload() takes up what another window saved (the flyout and mini player follow the main window)', () => {
    const mem = memory();
    const main = new ThemeStore(mem, fakeMq(true).mq);
    const flyout = new ThemeStore(mem, fakeMq(true).mq);
    expect(flyout.settings.themeId).toBe(main.settings.themeId);
    main.update({ followSystem: false, themeId: 'light' });
    expect(flyout.settings.themeId).not.toBe('light'); // not yet: it only learns on reload
    flyout.reload();
    expect(flyout.settings.followSystem).toBe(false);
    expect(flyout.settings.themeId).toBe('light');
    expect(flyout.active.id).toBe('light');
  });
});
