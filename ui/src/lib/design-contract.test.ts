import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// vitest runs from the ui folder
const css = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8');

const sources = import.meta.glob('../**/*.svelte', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const source = (name: string) => {
  const hit = Object.entries(sources).find(([path]) => path.endsWith(`/${name}`));
  if (!hit) throw new Error(`no source named ${name}`);
  return hit[1];
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
  it('Mica makes the page background transparent, which is why other tokens must not depend on it', () => {
    expect(rule(":root[data-backdrop='mica']")).toMatch(/--bg:\s*transparent/);
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
    expect(rule(':root')).toMatch(/--on-fg:\s*#0f0f0f/);
    expect(rule(":root[data-theme='light']")).toMatch(/--on-fg:\s*#ffffff/);
  });

  it('no component paints text or borders with the page background (the black-button bug)', () => {
    for (const [path, text] of Object.entries(sources)) {
      expect(text, path).not.toMatch(/\btext-bg\b|\bborder-bg\b|\bborder-t-bg\b/);
    }
  });

  it('overlays and menus use opaque backgrounds, so nothing shows through them', () => {
    expect(source('Sheet.svelte')).toContain('bg-solid-2');
    expect(source('Sheet.svelte')).not.toMatch(/bg-surface-2 p-4/);
    expect(source('TopBar.svelte')).toMatch(/id="search-suggestions"[^>]*bg-solid-2/);
    expect(source('NowPlaying.svelte')).toContain('var(--solid)');
    expect(source('NowPlaying.svelte')).not.toMatch(/style="background:[^"]*var\(--surface\)/);
  });

  it('exposes the opaque tokens to Tailwind utilities', () => {
    for (const t of ['solid', 'solid-2', 'on-fg']) expect(css).toContain(`--color-${t}: var(--${t})`);
  });
});
