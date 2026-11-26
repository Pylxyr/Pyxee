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

describe('the smooth seek-bar fill (PYXEE_SEEK_MODE=glide|steps)', () => {
  it('is moved only by a finite Web Animations transform: no CSS animation, transition or layer promotion on it', () => {
    for (const sel of ['.seek-wrap', '.seek-clip', '.seek-fill', '.seek-fill.accent', "input[type='range'].slider.glide"]) {
      const body = all.find((b) => b.selector === sel)?.body ?? '';
      expect(body, sel).toBeTruthy();
      expect(body, sel).not.toMatch(/transition|animation|will-change|filter/);
    }
    // the fill is scaled, never resized: a width change would lay the page out on every frame
    expect(all.find((b) => b.selector === '.seek-fill')!.body).toMatch(/transform:\s*scaleX\(0\)/);
    expect(all.find((b) => b.selector === '.seek-fill')!.body).not.toMatch(/\bwidth:\s*var/);
  });
});

describe('no large blur, no backdrop filter', () => {
  it('has no backdrop-filter, will-change or blend mode other than normal', () => {
    expect(css).not.toMatch(/backdrop-filter/);
    expect(css).not.toMatch(/will-change/);
    for (const m of css.matchAll(/mix-blend-mode:\s*([\w-]+)/g)) expect(m[1]).toBe('normal');
  });

  it('blurs nothing at run time: not in a rule, not in a keyframe', () => {
    for (const b of all) expect(b.body, `blur in ${b.selector}`).not.toMatch(/blur\(/);
  });

  it('keeps the soft backdrops to a small canvas the browser only stretches (see lib/backdrop.ts), not a filter', () => {
    for (const sel of ['.np-bg', '.hero-bg']) {
      const body = all.find((b) => b.selector === sel)?.body ?? '';
      expect(body, sel).toBeTruthy();
      expect(body, sel).not.toMatch(/filter/);
    }
  });
});

/** Colour changes on hover and press: small, one-off, ended by themselves, never part of the playing state. */
const PAINT_ONLY = new Set(['background-color', 'color', 'border-color', 'text-decoration-color']);

describe('motion only moves transform and opacity', () => {
  it('every keyframe a rule uses changes only transform and opacity', () => {
    const names = new Set(all.flatMap((b) => [...b.body.matchAll(/animation(?:-name)?:\s*([a-z][\w-]*)/g)].map((m) => m[1]!)));
    const known = [...css.matchAll(/@keyframes\s+([\w-]+)/g)].map((m) => m[1]!);
    expect(known.length).toBeGreaterThan(5);
    for (const name of known) {
      expect(names.has(name), `@keyframes ${name} is not used by any rule`).toBe(true);
      const props = keyframeProps(name);
      expect(props.filter((p) => p !== 'transform' && p !== 'opacity'), `${name} must only change transform/opacity`).toEqual([]);
    }
  });

  it('no transition moves a layout property, a filter or a shadow (colours on hover are the one allowance)', () => {
    const allowed = new Set(['transform', 'opacity', 'none', 'all', ...PAINT_ONLY]);
    const bad = all
      .flatMap((b) => transitionedProps(b.body).filter((p) => !allowed.has(p) && !p.startsWith('--')).map((p) => `${b.selector}: ${p}`));
    expect(bad).toEqual([]);
    // (the check itself)
    expect(transitionedProps('transition: width 200ms, opacity 1s;')).toEqual(['width', 'opacity']);
  });

  it('a transition on all is never used (it would pick up a layout change)', () => {
    expect(all.filter((b) => transitionedProps(b.body).includes('all')).map((b) => b.selector)).toEqual([]);
  });

  it('the covers glow and brighten on a layer that is drawn once: no animated shadow, no brightness filter', () => {
    expect(css).not.toMatch(/filter:\s*brightness/);
    expect(keyframeProps('glow-in')).toEqual(['opacity']);
  });
});

/** The properties a `transition` / `transition-property` declaration names. */
function transitionedProps(body: string): string[] {
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

describe('the seek-line wake (a new song lights the seek line)', () => {
  const rule = all.find((b) => b.selector === '.seek-wake i')?.body ?? '';

  it('moves only transform and opacity, once, and leaves nothing running or promoted afterwards', () => {
    expect(rule).toMatch(/animation:\s*wake\b/);
    expect(keyframeProps('wake').sort()).toEqual(['opacity', 'transform']);
    expect(rule).not.toMatch(/infinite|forwards|both|will-change|filter/);
    // at rest it is invisible, so after the animation nothing of it is drawn
    expect(rule).toMatch(/opacity:\s*0\b/);
  });

  it('stands down under reduced motion and in forced colours', () => {
    const reduced = /@media \(prefers-reduced-motion: reduce\) \{([\s\S]*?)animation: none !important;/.exec(css)?.[1] ?? '';
    expect(reduced).toContain('.seek-wake i');
    expect(css).toMatch(/@media \(forced-colors: active\) \{[\s\S]*?\.seek-wake \{\s*display: none;/);
  });
});

describe('the seek bar\'s time bubble and the paused cover', () => {
  it('fades the bubble in once, with opacity only, and stands down under reduced motion', () => {
    const rule = all.find((b) => b.selector === '.seek-tip')?.body ?? '';
    expect(rule).toMatch(/animation:\s*tip-in\b/);
    expect(rule).not.toMatch(/infinite|will-change|filter/);
    expect(keyframeProps('tip-in')).toEqual(['opacity']);
    expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\) \{\s*\.seek-tip \{\s*animation: none !important;/);
  });

  it('eases the Now Playing cover down with one transform transition, and not under reduced motion', () => {
    const rest = all.find((b) => b.selector === '.np-root .stage-art' && /(^|\s)transition:\s*transform/.test(b.body))?.body ?? '';
    const value = /transition:\s*([^;}]*)/.exec(rest)?.[1]?.trim() ?? '';
    expect(value.startsWith('transform ') && !value.includes(','), value).toBe(true);
    expect(all.find((b) => b.selector === ".np-root .stage-art[data-paused='true']")?.body).toMatch(/transform:\s*scale\(/);
    expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\) \{\s*\.np-root \.stage-art \{\s*transition: none;/);
  });
});

describe('the deck cover\'s light follows the music with opacity only', () => {
  const lit = all.find((b) => b.selector === '.deck-art::before')?.body ?? '';
  const dim = all.find((b) => b.selector === ".deck-art[data-paused='true']::before")?.body ?? '';

  it('dims by opacity on the pre-rendered glow layer: no animated shadow, no filter, no fill kept after the arrival', () => {
    expect(dim).toMatch(/^\s*opacity:\s*0?\.\d+;\s*$/);
    expect(transitionedProps(lit)).toEqual(['opacity']);
    expect(lit).toMatch(/animation:\s*glow-in\b[^;]*\bbackwards\b/);
    expect(lit).not.toMatch(/forwards|both|filter|will-change/);
    // the shadow itself is drawn once and never transitioned
    expect(transitionedProps(lit)).not.toContain('box-shadow');
  });

  it('its length comes from a token, so reduced motion (which zeroes the tokens) makes it instant', () => {
    expect(lit).toMatch(/transition:\s*opacity var\(--dur-slow\)/);
  });
});

describe('the snackbar time rail', () => {
  const rail = all.filter((b) => b.selector === '.toast-rail').find((b) => b.body.includes('height'))?.body ?? '';
  it('is one drawn bar that is moved by script (transform), with no CSS animation or promotion', () => {
    expect(rail).toMatch(/transform-origin:\s*left/);
    expect(rail).not.toMatch(/animation|transition|will-change|filter/);
    expect(rail).toMatch(/height:\s*2px/);
  });
  it('is hidden under reduced motion and in forced colours', () => {
    expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\) \{[\s\S]*?\.toast-rail \{\s*display: none;/);
    expect(css).toMatch(/@media \(forced-colors: active\) \{[\s\S]*?\.toast-rail \{\s*display: none;/);
  });
});

describe('the thrown song (toss to queue)', () => {
  const dot = all.find((b) => b.selector === '.toss-dot')?.body ?? '';
  it('is one fixed dot that lives only inside a script animation: no CSS animation, no promotion', () => {
    expect(dot).toMatch(/position:\s*fixed/);
    expect(dot).toMatch(/opacity:\s*0/);
    expect(dot).not.toMatch(/animation|transition|will-change|filter|box-shadow/);
  });
  it('is not drawn in forced colours, where the notice says it all', () => {
    expect(css).toMatch(/@media \(forced-colors: active\) \{[\s\S]*?\.toss-dot,\s*\.toast-rail \{\s*display: none;/);
  });
});

describe('the theme crossfade', () => {
  it('holds the accent glide off while the whole-window crossfade runs, and adds no animation of its own', () => {
    expect(css).toMatch(/:root\[data-theme-fade\] \{\s*transition: none;\s*\}/);
    // the crossfade is the root view transition's: its group length is set, and nothing else is styled for it
    expect(css).toMatch(/::view-transition-group\(root\) \{\s*animation-duration: 300ms;/);
  });
});

describe('the scroll edge under a pinned page header', () => {
  const edge = all.filter((b) => b.selector === '.page-edge::after').find((b) => b.body.includes('gradient'))?.body ?? '';
  it('is one gradient layer that only fades (opacity), with no animation timeline, filter or promotion', () => {
    expect(edge).toMatch(/linear-gradient/);
    expect(edge).toMatch(/transition:\s*opacity var\(--dur-base\)/);
    expect(edge).not.toMatch(/animation|will-change|filter|box-shadow/);
    // a scroll-linked animation would count as a running animation while a song plays: not used anywhere
    expect(css).not.toMatch(/animation-timeline|scroll-timeline|timeline-scope/);
  });
  it('is not drawn in forced colours (the header keeps its hairline)', () => {
    expect(css).toMatch(/@media \(forced-colors: active\) \{[\s\S]*?\.page-edge::after \{\s*display: none;/);
  });
});

describe('rows and covers that reveal themselves on scroll', () => {
  const [waiting, reduced] = all.filter((b) => b.selector === '[data-reveal]');

  it('wait as one invisible state with nothing running: opacity only, no animation, transition, promotion or filter', () => {
    expect(waiting?.body).toMatch(/^\s*opacity:\s*0;\s*$/);
    expect(reduced?.body).toMatch(/^\s*opacity:\s*1;\s*$/);
    // the animation itself is a one-shot from script (lib/motion.ts), so nothing in the CSS may animate, or keep a layer
    for (const b of [waiting, reduced]) expect(b?.body).not.toMatch(/animation|transition|will-change|filter/);
  });

  it('are drawn as they are under reduced motion, in case the setting changed after they were marked', () => {
    expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\) \{[\s\S]*?\[data-reveal\] \{\s*opacity: 1;/);
  });

  it('use no scroll-linked animation: the reveal is an observer that lets go of a row once it has played', () => {
    expect(css).not.toMatch(/animation-timeline|view-timeline/);
  });
});

describe('the failure nudge', () => {
  const rule = all.find((b) => b.selector === '.fail-in')?.body ?? '';
  it('is a one-off transform and opacity shake with no fill kept, and stands down under reduced motion', () => {
    expect(rule).toMatch(/animation:\s*nudge\b/);
    expect(keyframeProps('nudge').sort()).toEqual(['opacity', 'transform']);
    expect(rule).not.toMatch(/infinite|forwards|both|will-change/);
    const reduced = /@media \(prefers-reduced-motion: reduce\) \{([\s\S]*?)animation: none !important;/.exec(css)?.[1] ?? '';
    expect(reduced).toContain('.fail-in');
  });
});
