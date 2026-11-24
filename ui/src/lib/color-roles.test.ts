import { describe, expect, it } from 'vitest';
import { centreSquare, contrast, coverRoles, dominantColor, dominantPair, mixHex, paletteFromRoles, readableOn, type CoverRoles } from './color';

type Rgb = [number, number, number];
/** A picture from colours and how many pixels each covers (in blocks: where a pixel sits does not matter to the reading). */
const paint = (...parts: [Rgb, number][]): Uint8ClampedArray => new Uint8ClampedArray(parts.flatMap(([c, n]) => Array.from({ length: n }, () => [...c, 255]).flat()));

const hex = ([r, g, b]: Rgb) => `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
const channels = (h: string): Rgb => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)) as Rgb;
const spread = (h: string) => Math.max(...channels(h)) - Math.min(...channels(h));
/** Hue (hsl, degrees) of a colour that has one. */
function hueOf(h: string): number {
  const [r, g, b] = channels(h).map((v) => v / 255) as Rgb;
  const max = Math.max(r, g, b), d = max - Math.min(r, g, b);
  if (d === 0) return 0;
  const k = max === r ? ((g - b) / d + (g < b ? 6 : 0)) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return k * 60;
}
const apart = (a: number, b: number) => Math.min(Math.abs(a - b), 360 - Math.abs(a - b));

/** Covers with the colour structure of real ones. `colours` are the hues a theme made from it may use. */
interface Fixture {
  name: string;
  data: Uint8ClampedArray;
  colours: Rgb[];
}
const WHITE: Rgb = [250, 250, 250];
const BLACK: Rgb = [14, 14, 14];
const fixtures: Fixture[] = [
  // Weather Station: a grey sleeve, black hair, white type, a touch of warm skin and brown shoes.
  { name: 'grey, white and black', data: paint([[176, 176, 176], 550], [BLACK, 180], [[60, 60, 60], 50], [WHITE, 120], [[250, 225, 215], 40], [[74, 50, 48], 10]), colours: [] },
  // TETORIS: a yellow cover with a red character, white type, light and dark grey, pale skin.
  { name: 'yellow with red', data: paint([[252, 215, 90], 600], [[236, 81, 111], 90], [[255, 255, 255], 100], [[190, 190, 190], 60], [[74, 74, 74], 50], [[253, 238, 230], 40]), colours: [[252, 215, 90], [236, 81, 111]] },
  // Zeroth Directive: pure yellow with white and black type.
  { name: 'yellow with white and black type', data: paint([[240, 240, 60], 520], [[237, 237, 237], 300], [[37, 37, 37], 120], [[150, 150, 150], 60]), colours: [[240, 240, 60]] },
  { name: 'one pure hue', data: paint([[40, 110, 230], 1000]), colours: [[40, 110, 230]] },
  { name: 'black only', data: paint([[0, 0, 0], 1000]), colours: [] },
  { name: 'white only', data: paint([[255, 255, 255], 1000]), colours: [] },
  { name: 'a pale tinted cover', data: paint([[244, 230, 236], 700], [[255, 255, 255], 300]), colours: [[244, 230, 236]] },
  { name: 'three clear hues', data: paint([[220, 40, 50], 300], [[40, 170, 80], 300], [[40, 90, 220], 300], [BLACK, 100]), colours: [[220, 40, 50], [40, 170, 80], [40, 90, 220]] },
  // A black cover with a small red logo.
  { name: 'black with a small red logo', data: paint([[10, 10, 12], 900], [[200, 30, 40], 60], [[240, 240, 240], 40]), colours: [[200, 30, 40]] },
];
const byName = (name: string) => fixtures.find((f) => f.name === name)!;
const rolesOf = (f: Fixture): CoverRoles => coverRoles(f.data)!;
const schemes = ['dark', 'light'] as const;

describe('coverRoles', () => {
  it('has nothing for a picture with nothing in it', () => {
    expect(coverRoles(new Uint8ClampedArray())).toBeNull();
    expect(coverRoles(paint([[200, 40, 60], 20]).map((v, i) => (i % 4 === 3 ? 0 : v)))).toBeNull();
  });

  it('wears the colours dominantPair finds, so a hue is only ever one the picture has', () => {
    for (const f of fixtures) {
      const r = rolesOf(f), pair = dominantPair(f.data)!;
      expect(r.primary, f.name).toBe(dominantColor(f.data));
      expect(r.primary, f.name).toBe(pair.a);
      expect(r.secondary, f.name).toBe(pair.b);
      if (pair.c) expect(r.tertiary, f.name).toBe(pair.c);
    }
  });

  it('gives a grey, black or white cover no hue at all: every role is a grey', () => {
    for (const name of ['grey, white and black', 'black only', 'white only']) {
      const r = rolesOf(byName(name));
      expect(r.hued, name).toBe(false);
      for (const c of [r.primary, r.secondary, r.tertiary]) expect(spread(c), `${name} ${c}`).toBeLessThanOrEqual(12);
    }
  });

  it('finds the dark and the light of a grey cover: the black hair and the white type', () => {
    const r = rolesOf(byName('grey, white and black'));
    expect(channels(r.dark!)[0]).toBeLessThan(0x30);
    expect(channels(r.light!)[0]).toBeGreaterThan(0xd8);
    expect(rolesOf(byName('black only')).light).toBeNull();
    expect(rolesOf(byName('white only')).dark).toBeNull();
  });

  it('gives a yellow cover with a red character both colours, and its white as the third', () => {
    const r = rolesOf(byName('yellow with red'));
    expect(r.hued).toBe(true);
    expect(apart(hueOf(r.primary), 46)).toBeLessThan(12);
    expect(apart(hueOf(r.secondary), 350)).toBeLessThan(20);
    expect(spread(r.tertiary)).toBeLessThan(25); // white, not a hue made up
    expect(r.colourShare).toBeGreaterThan(0.6);
  });

  it('gives one hue its own dark and its own light, not another hue', () => {
    const r = rolesOf(byName('one pure hue'));
    for (const c of [r.secondary, r.tertiary]) expect(apart(hueOf(c), hueOf(r.primary)), c).toBeLessThan(12);
    expect(new Set([r.primary, r.secondary, r.tertiary]).size).toBe(3);
  });

  it('gives three clear hues to the three roles', () => {
    const r = rolesOf(byName('three clear hues'));
    const hues = [r.primary, r.secondary, r.tertiary].map(hueOf);
    expect(Math.min(apart(hues[0]!, hues[1]!), apart(hues[0]!, hues[2]!), apart(hues[1]!, hues[2]!))).toBeGreaterThanOrEqual(45);
  });

  it('counts how much of the picture is coloured', () => {
    expect(rolesOf(byName('black with a small red logo')).colourShare).toBeLessThan(0.15);
    expect(rolesOf(byName('grey, white and black')).colourShare).toBe(0);
  });

  it('reads the cover art of a 16:9 still, not the bars beside it', () => {
    const art = byName('yellow with red').data;
    const side = 90; // the art is 90 x 90 in a 160 x 90 still, between two bars of a dark colour taken from the art
    const w = 160, h = 90, bar: Rgb = [70, 60, 20];
    const still = new Uint8ClampedArray(w * h * 4);
    const left = (w - side) / 2;
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const inside = x >= left && x < left + side;
        const px = inside ? [...art.slice((y * side + (x - left)) * 4, (y * side + (x - left)) * 4 + 4)] : [...bar, 255];
        still.set(px, (y * w + x) * 4);
      }
    }
    const { sx, sy, size } = centreSquare(w, h);
    const square = new Uint8ClampedArray(size * size * 4);
    for (let y = 0; y < size; y++) square.set(still.slice(((sy + y) * w + sx) * 4, ((sy + y) * w + sx + size) * 4), y * size * 4);
    expect(coverRoles(square)).toEqual(coverRoles(art));
    expect(coverRoles(still)).not.toEqual(coverRoles(art));
  });
});

describe('paletteFromRoles', () => {
  it('makes a cover without a hue grey, white and near-black, and nothing else', () => {
    for (const name of ['grey, white and black', 'black only', 'white only']) {
      for (const scheme of schemes) {
        const p = paletteFromRoles(rolesOf(byName(name)), scheme);
        for (const [key, c] of Object.entries({ ...p.colors, accent2: p.accent2, accent3: p.accent3 })) expect(spread(c), `${name} ${scheme} ${key} ${c}`).toBeLessThanOrEqual(1);
      }
    }
  });

  it('stands the greys of a grey cover apart: near-white, mid grey and near-black do different jobs', () => {
    const dark = paletteFromRoles(rolesOf(byName('grey, white and black')), 'dark');
    expect(channels(dark.colors.bg)[0]).toBeLessThan(0x30);
    expect(channels(dark.colors.fg)[0]).toBeGreaterThan(0xe8);
    const lum = (c: string) => contrast(c, '#000000');
    expect(lum(dark.accent3)).toBeGreaterThan(lum(dark.colors.accent) + 1.5);
    expect(lum(dark.colors.accent)).toBeGreaterThan(lum(dark.accent2) + 1.5);
    const light = paletteFromRoles(rolesOf(byName('grey, white and black')), 'light');
    expect(channels(light.colors.bg)[0]).toBeGreaterThan(0xe0);
    expect(contrast(light.accent2, '#000000')).toBeLessThan(contrast(light.colors.accent, '#000000'));
  });

  it('uses both the yellow and the red of a yellow cover with a red character, in both schemes, and never a green', () => {
    for (const scheme of schemes) {
      const p = paletteFromRoles(rolesOf(byName('yellow with red')), scheme);
      expect(hueOf(p.colors.accent), scheme).toBeGreaterThan(36);
      expect(hueOf(p.colors.accent), scheme).toBeLessThan(60);
      expect(apart(hueOf(p.accent2), 350), scheme).toBeLessThan(20);
      expect(spread(p.colors.accent), scheme).toBeGreaterThan(80);
      expect(spread(p.accent2), scheme).toBeGreaterThan(80);
    }
  });

  it('keeps yellow gold in the light scheme: its darker shades are not olive', () => {
    for (const name of ['yellow with red', 'yellow with white and black type']) {
      const p = paletteFromRoles(rolesOf(byName(name)), 'light');
      for (const c of [p.colors.accent, p.accent2, p.accent3]) {
        // A yellow-green hue after 58 degrees is olive; reds and greys are not in question.
        if (spread(c) > 40 && hueOf(c) > 36 && hueOf(c) < 150) expect(hueOf(c), `${name} ${c}`).toBeLessThan(58);
      }
    }
  });

  it('gives a one-hue cover accents of its own hue, and surfaces that are tinted by it', () => {
    for (const scheme of schemes) {
      const p = paletteFromRoles(rolesOf(byName('one pure hue')), scheme);
      for (const c of [p.colors.accent, p.accent2, p.accent3]) expect(apart(hueOf(c), 217), `${scheme} ${c}`).toBeLessThan(14);
      expect(spread(p.colors.bg), scheme).toBeGreaterThan(8);
      expect(p.accent2, scheme).not.toBe(p.colors.accent);
    }
    const dark = paletteFromRoles(rolesOf(byName('one pure hue')), 'dark');
    expect(new Set([dark.colors.accent, dark.accent2, dark.accent3]).size).toBe(3);
  });

  it('invents no hue: every accent that has one is a hue of the picture', () => {
    for (const f of fixtures) {
      for (const scheme of schemes) {
        const p = paletteFromRoles(rolesOf(f), scheme);
        for (const c of [p.colors.accent, p.accent2, p.accent3]) {
          if (spread(c) < 30) continue; // grey, black or white: no hue
          const near = Math.min(...f.colours.map((x) => apart(hueOf(c), hueOf(hex(x)))));
          // 25 degrees: a dark yellow is pulled toward amber on purpose (deepHue), and a saturated hue keeps its place.
          expect(near, `${f.name} ${scheme} ${c}`).toBeLessThanOrEqual(25);
        }
      }
    }
  });

  it('tints the surfaces only as far as the picture is coloured', () => {
    const logo = paletteFromRoles(rolesOf(byName('black with a small red logo')), 'dark').colors.bg;
    const yellow = paletteFromRoles(rolesOf(byName('yellow with red')), 'dark').colors.bg;
    expect(spread(logo)).toBeLessThan(spread(yellow) / 2);
    expect(spread(paletteFromRoles(rolesOf(byName('grey, white and black')), 'dark').colors.bg)).toBe(0);
  });

  describe('contrast, for every cover in both schemes', () => {
    for (const f of fixtures) {
      for (const scheme of schemes) {
        it(`${f.name}, ${scheme}`, () => {
          const { colors: c, accent2, accent3 } = paletteFromRoles(rolesOf(f), scheme);
          const grounds = { bg: c.bg, surface: c.surface, 'raised panels': c.surface2 };
          const roles = { primary: c.accent, secondary: accent2, tertiary: accent3 };
          const failures: string[] = [];
          const need = (what: string, a: string, b: string, min: number) => {
            const ratio = contrast(a, b);
            if (ratio < min) failures.push(`${what}: ${a} on ${b} is ${ratio.toFixed(2)}:1, wants ${min}`);
          };
          for (const [ground, g] of Object.entries(grounds)) {
            need(`text on ${ground}`, c.fg, g, 4.5);
            need(`secondary text on ${ground}`, c.muted, g, 4.5);
          }
          for (const [role, x] of Object.entries(roles)) {
            // Fills, bars, icons and the focus ring (3:1), on every surface and on the selected row's tint of the primary.
            const row = role === 'secondary' ? {} : { 'the selected row': mixHex(c.surface2, c.accent, 0.16) }; // the secondary only tints it as a sweep
            for (const [ground, g] of Object.entries({ ...grounds, ...row })) need(`${role} on ${ground}`, x, g, 3);
            // The text on a role's fill.
            need(`text on ${role}`, readableOn(x), x, 4.5);
          }
          // The primary used as text (accent-ink, a quarter of the text colour in it) on the page, a panel and the selected row.
          const ink = mixHex(c.fg, c.accent, 0.75);
          for (const [ground, g] of Object.entries({ bg: c.bg, surface: c.surface, 'the selected row': mixHex(c.surface2, c.accent, 0.16) })) need(`ink on ${ground}`, ink, g, 4.5);
          // A gradient from the primary to the secondary carries one text colour: it must read at every point of it.
          for (let i = 0; i <= 8; i++) need(`text on the gradient at ${i}/8`, readableOn(c.accent), mixHex(c.accent, accent2, i / 8), 4.5);
          expect(failures).toEqual([]);
        });
      }
    }
  });
});
