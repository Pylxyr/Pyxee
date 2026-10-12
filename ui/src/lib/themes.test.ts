import { describe, expect, it } from 'vitest';
import { contrast } from './color';
import {
  BUILTIN_THEMES, contrastChecks, exportTheme, parseThemeText, sanitizeTheme, themeVars, type ThemeDef,
} from './themes';

const lum = (hex: string) => contrast(hex, '#000000');

describe('built-in themes', () => {
  it('include Dark, Light and YT Music, with unique ids', () => {
    const ids = BUILTIN_THEMES.map((x) => x.id);
    expect(ids).toEqual(expect.arrayContaining(['dark', 'light', 'ytmusic']));
    expect(new Set(ids).size).toBe(ids.length);
    expect(BUILTIN_THEMES.length).toBeGreaterThanOrEqual(8);
  });

  for (const def of BUILTIN_THEMES) {
    it(`${def.name} is readable`, () => {
      const failing = contrastChecks(def.colors).filter((c) => !c.ok).map((c) => `${c.label}: ${c.ratio.toFixed(2)} < ${c.min}`);
      expect(failing).toEqual([]);
    });

    it(`${def.name} really is a ${def.scheme} theme`, () => {
      const dark = lum(def.colors.bg) < lum(def.colors.fg);
      expect(dark).toBe(def.scheme === 'dark');
    });

    it(`${def.name} has readable text on its filled buttons`, () => {
      const v = themeVars(def.colors);
      expect(contrast(v['--t-on-fg']!, def.colors.fg)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(v['--t-on-accent']!, def.colors.accent)).toBeGreaterThanOrEqual(4.5);
    });
  }
});

describe('themeVars', () => {
  it('keeps the solid tokens opaque copies of the page and raised colours', () => {
    const v = themeVars(BUILTIN_THEMES[0]!.colors);
    expect(v['--t-solid']).toBe(BUILTIN_THEMES[0]!.colors.bg);
    expect(v['--t-solid-2']).toBe(BUILTIN_THEMES[0]!.colors.surface2);
  });
});

describe('sharing themes', () => {
  const mine: ThemeDef = { id: 'custom-1', name: 'Mine', scheme: 'dark', colors: { ...BUILTIN_THEMES[0]!.colors, accent: '#00ff88' } };

  it('round-trips through export and import, getting a fresh id', () => {
    const back = parseThemeText(exportTheme(mine));
    expect(back).toEqual({ ...mine, id: '' });
  });
  it('accepts short and upper-case hex colours', () => {
    const def = sanitizeTheme({ name: 'x', scheme: 'light', colors: { ...mine.colors, accent: '#0F8', bg: 'FFFFFF' } });
    expect(def?.colors.accent).toBe('#00ff88');
    expect(def?.colors.bg).toBe('#ffffff');
  });
  it('refuses anything incomplete or not a theme, with a sentence to show', () => {
    expect(() => parseThemeText('nope')).toThrow(/isn't theme text/);
    expect(() => parseThemeText('{"name":"x"}')).toThrow(/missing/);
    expect(() => parseThemeText(JSON.stringify({ name: 'x', scheme: 'dark', colors: { ...mine.colors, fg: 'red' } }))).toThrow(/missing/);
    expect(sanitizeTheme(null)).toBeNull();
    expect(sanitizeTheme({ name: '  ', scheme: 'dark', colors: mine.colors })).toBeNull();
  });
  it('cuts a very long name', () => {
    expect(sanitizeTheme({ name: 'a'.repeat(100), scheme: 'dark', colors: mine.colors })?.name).toHaveLength(40);
  });
});
