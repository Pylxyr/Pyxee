import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { BUILTIN_THEMES, themeVars } from './themes';

// vitest runs from the ui folder
// A Windows checkout may have CRLF line endings (core.autocrlf); the selectors below span lines with LF.
const lf = (text: string) => text.replace(/\r\n/g, '\n');
const css = lf(readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8'));

const sources = import.meta.glob('../**/*.svelte', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const source = (name: string) => {
  const hit = Object.entries(sources).find(([path]) => path.endsWith(`/${name}`));
  if (!hit) throw new Error(`no source named ${name}`);
  return lf(hit[1]);
};

/** The text of the first CSS rule whose selector is exactly `selector`. */
function rule(selector: string): string {
  const start = css.indexOf(`${selector} {`);
  if (start < 0) throw new Error(`no CSS rule for ${selector}`);
  return css.slice(start, css.indexOf('}', start));
}

// These guard against the "black buttons / see-through overlay" bugs seen on Windows 11, where Mica makes the
// page background transparent. Anything that must stay readable or opaque may not depend on it.
describe('Mica-safe colours', () => {
  it('Mica makes a grey theme background transparent (a coloured one tints it), which is why other tokens must not depend on it', () => {
    expect(rule(":root[data-backdrop='mica']")).toMatch(/--bg:\s*color-mix\(in srgb, var\(--t-bg[^)]*\) var\(--t-mica-tint, 0%\), transparent\)/);
    const tint = (id: string) => themeVars(BUILTIN_THEMES.find((t) => t.id === id)!.colors)['--t-mica-tint'];
    for (const grey of ['light', 'oled']) expect(tint(grey), grey).toBe('0%');
    for (const coloured of ['dark', 'deepblue', 'sky', 'ember', 'paper']) expect(tint(coloured), coloured).toBe('70%');
  });

  it('every variable a theme writes is read by the stylesheet', () => {
    for (const name of Object.keys(themeVars(BUILTIN_THEMES[0]!.colors))) expect(css, name).toContain(`var(${name}`);
  });

  it('opaque tokens exist for both themes and Mica never overrides them', () => {
    for (const token of ['--solid', '--solid-2', '--on-fg']) {
      expect(rule(':root'), token).toContain(`${token}:`);
      expect(rule(":root[data-theme='light']"), token).toContain(`${token}:`);
      expect(rule(":root[data-backdrop='mica']"), token).not.toContain(`${token}:`);
      expect(rule(":root[data-backdrop='mica'][data-theme='light']"), token).not.toContain(`${token}:`);
    }
  });

  it('text on a solid foreground button is dark on dark theme and light on light theme', () => {
    expect(rule(':root')).toMatch(/--on-fg:\s*var\(--t-on-fg, #0f0f0f\)/);
    expect(rule(":root[data-theme='light']")).toMatch(/--on-fg:\s*var\(--t-on-fg, #ffffff\)/);
  });

  it('no component paints text or borders with the page background (the black-button bug)', () => {
    for (const [path, text] of Object.entries(sources)) {
      expect(text, path).not.toMatch(/\btext-bg\b|\bborder-bg\b|\bborder-t-bg\b/);
    }
  });

  it('a drop-down list is opaque and readable: its colour comes from --solid-2, never from the see-through --surface alone', () => {
    const select = rule('select.field');
    expect(select).toMatch(/background-color:\s*var\(--solid-2\)/);
    expect(select).toMatch(/background-image:\s*linear-gradient\(var\(--surface\), var\(--surface\)\)/);
    const option = rule('select.field option,\n  select.field optgroup');
    expect(option).toMatch(/background-color:\s*var\(--solid-2\)/);
    expect(option).toMatch(/color:\s*var\(--fg\)/);
  });

  it('overlays and menus use opaque backgrounds, so nothing shows through them', () => {
    expect(source('Sheet.svelte')).toContain('bg-solid-2');
    expect(source('Sheet.svelte')).not.toMatch(/bg-surface-2 p-4/);
    expect(source('TopBar.svelte')).toMatch(/id="search-suggestions"[^>]*bg-solid-2/);
    expect(source('NowPlaying.svelte')).not.toMatch(/style="background:/);
  });

  it('Now Playing is opaque under every look of its background (they follow the top card), because it covers the whole window', () => {
    // The base (blur rests on it), then each look that replaces it: all rest on `--solid`, never on `--bg`, `--surface` or `--layer`.
    for (const sel of [".np-root[data-wash='plain']", ".np-wash[data-wash='aurora']", ".np-wash[data-wash='gradient']", ".np-wash[data-wash='vinyl']"]) {
      const r = rule(sel);
      expect(r, sel).toContain('var(--solid)');
      expect(r, sel).not.toMatch(/var\(--(bg|surface|layer)\)/);
    }
    const base = css.match(/\.np-root \{\s*background:[^}]*\}/)?.[0] ?? '';
    expect(base).toContain('var(--solid)');
    expect(base).not.toMatch(/var\(--(bg|surface|layer)\)/);
  });

  it("the cover-colored looks of Home's top card and the Tune in cards are layers that fade, never a transition of their colors", () => {
    for (const sel of [".hero-wash[data-wash='aurora']", ".hero-wash[data-wash='gradient']", ".hero-wash[data-wash='vinyl']", '.mix-bg']) {
      const r = rule(sel);
      expect(r, sel).toContain('background');
      expect(r, sel).not.toMatch(/transition/);
    }
    // the card itself keeps no look of its own that depends on the cover's colors changing
    expect(css).not.toMatch(/\.hero\[data-hero='(gradient|vinyl)'\]\s*\{/);
    expect(css).not.toMatch(/@property --(hero|mix)-/);
    expect(rule('.wash')).toMatch(/animation:\s*fade-in/);
  });

  it('exposes the opaque tokens to Tailwind utilities', () => {
    for (const t of ['solid', 'solid-2', 'on-fg']) expect(css).toContain(`--color-${t}: var(--${t})`);
  });
});

describe('scroll bars', () => {
  it('are thin and tinted from theme tokens, never a fixed colour', () => {
    const bars = css.slice(css.indexOf('::-webkit-scrollbar {'));
    expect(css).toMatch(/scrollbar-width:\s*thin/);
    // The standard properties are only a fallback for an engine without ::-webkit-scrollbar (they would override it).
    expect(css).toMatch(/@supports not selector\(::-webkit-scrollbar\)\s*\{[^}]*scrollbar-color:\s*color-mix\(in srgb, var\(--accent\)/);
    expect(bars).toContain('::-webkit-scrollbar-thumb');
    expect(bars).toMatch(/::-webkit-scrollbar-thumb \{[^}]*color-mix\(in srgb, var\(--fg\)/);
    const block = css.slice(css.indexOf('/*\n * Scroll bars'), css.indexOf('[data-scroll]:focus-visible'));
    expect(block).not.toMatch(/#[0-9a-f]{3,8}\b/i);
  });

  it('the Queue and Now Playing lists scroll in areas the keyboard can reach', () => {
    expect(source('ScrollPage.svelte')).toMatch(/data-scroll[^>]*tabindex="0"/);
    expect(source('NowPlaying.svelte')).toMatch(/data-testid="np-scroll" data-scroll tabindex="0"/);
  });
});
