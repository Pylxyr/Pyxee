import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

/**
 * A playing window must be able to go quiet: no frame is drawn unless something visibly changes. Anything that
 * runs without end, or that makes the browser blur a large area, keeps the page (and the GPU) busy for as long as a
 * song plays: measured at 90% of a core in a software renderer for the seek bar's custom-property transition and the
 * equalizer bars alone, and a further ~25 points for the full-window blur (`scripts/perf/measure.py`).
 * These tests fail when one comes back. An exception goes in an allow-list below WITH its reason.
 */

// vitest runs from the ui folder
const raw = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8');
const css = raw.replace(/\/\*[\s\S]*?\*\//g, '');
const sources = import.meta.glob('../**/*.svelte', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

/** Innermost `selector { declarations }` blocks (keyframe steps included), with the at-rule they sit in, if any. */
function blocks(text: string): { selector: string; body: string; at: string }[] {
  const out: { selector: string; body: string; at: string }[] = [];
  const re = /([^{}]+)\{([^{}]*)\}/g;
  for (const m of text.matchAll(re)) {
    const before = text.slice(0, m.index);
    // The at-rule this block is in: the last unclosed `@keyframes name {`.
    let depth = 0;
    let at = '';
    for (let i = before.length - 1; i >= 0; i--) {
      if (before[i] === '}') depth++;
      else if (before[i] === '{') {
        if (depth === 0) {
          const head = before.slice(before.lastIndexOf('}', i) + 1, i).trim();
          if (head.startsWith('@keyframes')) {
            at = head;
            break;
          }
        } else depth--;
      }
    }
    out.push({ selector: m[1]!.trim().replace(/\s+/g, ' '), body: m[2]!, at });
  }
  return out;
}
const all = blocks(css);

/** The CSS properties a `@keyframes name` sets. */
function keyframeProps(name: string): string[] {
  return [...new Set(all.filter((b) => b.at === `@keyframes ${name}`).flatMap((b) => [...b.body.matchAll(/([a-z-]+)\s*:/g)].map((m) => m[1]!)))];
}

/** Animations that run without end, by the rule that starts them. Each is transform- or opacity-only (checked below) and
 *  exists only while something is genuinely waiting, never in the state a playing, healthy window is in. */
const ENDLESS_ALLOWED: Record<string, { animation: string; why: string }> = {
  '.skeleton::after': { animation: 'sheen', why: 'placeholder shimmer: only while a list is loading (a finite wait); transform only' },
  '.uprog-track.indeterminate .uprog-fill': { animation: 'uprog-slide', why: 'update download whose size is not known: only during an update; transform only' },
  '.spinner': { animation: 'spin', why: 'play button while a song is loading (a finite wait); transform only' },
};

describe('nothing runs without end that the page does not need', () => {
  const endless = all.filter((b) => /\binfinite\b/.test(b.body));

  it('only the allow-listed waiting states animate forever, and only with transform or opacity', () => {
    const unexpected = endless.filter((b) => !(b.selector in ENDLESS_ALLOWED)).map((b) => b.selector);
    expect(unexpected, 'a never-ending animation: allow-list it with a reason, or make it finish').toEqual([]);
    for (const b of endless) {
      const name = /animation(?:-name)?:\s*([\w-]+)/.exec(b.body)?.[1];
      expect(name, b.selector).toBe(ENDLESS_ALLOWED[b.selector]!.animation);
      const props = keyframeProps(name!);
      expect(props.length, `${name} has keyframes`).toBeGreaterThan(0);
      expect(props.filter((p) => p !== 'transform' && p !== 'opacity'), `${name} must only change transform/opacity`).toEqual([]);
    }
  });

  it('every allow-list entry is still used (a stale entry would hide a future exception)', () => {
    for (const selector of Object.keys(ENDLESS_ALLOWED)) expect(endless.some((b) => b.selector === selector), selector).toBe(true);
  });

  /** The properties a `transition` / `transition-property` declaration names (not the `var(--x)` it may use). */
  function transitioned(body: string): string[] {
    const out: string[] = [];
    for (const m of body.matchAll(/transition(?:-property)?:\s*([^;]+)/g)) {
      let depth = 0;
      let item = '';
      const items: string[] = [];
      for (const ch of m[1]!) {
        if (ch === '(') depth++;
        if (ch === ')') depth--;
        if (ch === ',' && depth === 0) {
          items.push(item);
          item = '';
        } else item += ch;
      }
      items.push(item);
      out.push(...items.map((i) => i.trim().split(/\s+/)[0]!));
    }
    return out;
  }

  /** Custom-property transitions that are allowed: each ends by itself and is not part of the playing state. */
  const CUSTOM_TRANSITIONS_ALLOWED: Record<string, string> = {
    ':root': '--accent: once, 600 ms, when the colour taken from a new song arrives; it ends by itself (it does restyle the page each frame meanwhile)',
  };

  it('no transition on a custom property: it runs on the main thread and redraws every frame it lasts (the seek bar did)', () => {
    const bad = all
      .filter((b) => transitioned(b.body).some((p) => p.startsWith('--')) && !(b.selector in CUSTOM_TRANSITIONS_ALLOWED))
      .map((b) => b.selector);
    expect(bad).toEqual([]);
    // (the check itself)
    expect(transitioned('transition: --pct 260ms linear, opacity var(--dur-fast) var(--ease);')).toEqual(['--pct', 'opacity']);
  });

  it('the seek bar itself does not animate its fill', () => {
    for (const b of all.filter((x) => /^input\[type='range'\]\.slider(:[\w-]+)?$/.test(x.selector))) {
      expect(b.body, b.selector).not.toMatch(/transition|animation/);
    }
  });
});

describe('no large blur, no backdrop filter', () => {
  it('has no backdrop-filter, will-change or blend mode other than normal', () => {
    expect(css).not.toMatch(/backdrop-filter/);
    expect(css).not.toMatch(/will-change/);
    for (const m of css.matchAll(/mix-blend-mode:\s*([\w-]+)/g)) expect(m[1]).toBe('normal');
  });

  it('blurs only a few pixels, and only in a one-off keyframe', () => {
    for (const b of all) {
      for (const m of b.body.matchAll(/blur\(\s*([^)]*)\)/g)) {
        const px = parseFloat(m[1]!);
        expect(b.at, `blur in ${b.selector}: a blur belongs in a keyframe that finishes`).toMatch(/^@keyframes (focus-in)$/);
        expect(px, `${b.selector}: blur(${m[1]})`).toBeLessThanOrEqual(8);
      }
    }
  });

  it('keeps the soft backdrops to a small canvas the browser only stretches (see lib/backdrop.ts), not a filter', () => {
    for (const sel of ['.np-bg', '.hero-bg']) {
      const body = all.find((b) => b.selector === sel)?.body ?? '';
      expect(body, sel).toBeTruthy();
      expect(body, sel).not.toMatch(/filter/);
    }
  });
});

describe('components', () => {
  it('have no style block or utility class that animates forever, blurs, or promotes layers', () => {
    for (const [path, text] of Object.entries(sources)) {
      expect(text, path).not.toMatch(/\banimate-(spin|pulse|bounce|ping)\b/);
      expect(text, path).not.toMatch(/\bbackdrop-blur\b|\bbackdrop-filter\b|\b(?:-)?blur-(?:\d|\[|sm|md|lg|xl|2xl|3xl)/);
      expect(text, path).not.toMatch(/will-change/);
      for (const style of text.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)) {
        expect(style[1], path).not.toMatch(/infinite|filter:\s*blur|backdrop-filter/);
      }
    }
  });
});
