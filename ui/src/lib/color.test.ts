import { describe, expect, it } from 'vitest';
import { cardTones, centreSquare, contrast, deepHue, dominantColor, dominantPair, ensureContrast, heroTones, paletteFromArt } from './color';

function pixels(...px: [number, number, number, number][]): Uint8ClampedArray {
  return new Uint8ClampedArray(px.flat());
}
const repeat = (p: [number, number, number, number], n: number) => Array.from({ length: n }, () => p);

describe('dominantColor', () => {
  it('prefers the vivid colour over a larger grey area', () => {
    const data = pixels(...repeat([120, 120, 120, 255], 60), ...repeat([200, 40, 60, 255], 40));
    const hex = dominantColor(data)!;
    const r = parseInt(hex.slice(1, 3), 16), g = parseInt(hex.slice(3, 5), 16);
    expect(r).toBeGreaterThan(g + 80); // clearly red-ish
  });
  it('ignores transparent pixels, and near-black and near-white ones when there is anything else', () => {
    const data = pixels(...repeat([0, 0, 0, 255], 20), ...repeat([255, 255, 255, 255], 20), ...repeat([200, 40, 60, 255], 5), ...repeat([200, 40, 60, 0], 20));
    expect(dominantColor(data)).toBe('#c8283c');
  });
  it('gives a grey, not a hue, to a cover that is only dark or only light', () => {
    for (const px of [[20, 22, 30], [240, 240, 235], [0, 0, 0], [255, 255, 255]] as const) {
      const hex = dominantColor(pixels(...repeat([...px, 255], 50)))!;
      expect(hex.slice(1, 3) === hex.slice(3, 5) && hex.slice(3, 5) === hex.slice(5, 7), hex).toBe(true);
    }
  });
  it('has no colour only for a picture with nothing in it (transparent)', () => {
    expect(dominantColor(pixels(...repeat([200, 40, 60, 0], 20)))).toBeNull();
  });
  it('still finds a colour in a dark or a pale cover instead of giving up', () => {
    const dark = dominantColor(pixels(...repeat([24, 26, 40, 255], 50), ...repeat([0, 0, 0, 255], 50)));
    expect(dark).not.toBeNull();
    expect(parseInt(dark!.slice(5, 7), 16)).toBeGreaterThan(parseInt(dark!.slice(1, 3), 16)); // bluish
    const pale = dominantColor(pixels(...repeat([244, 238, 238, 255], 50), ...repeat([255, 255, 255, 255], 50)));
    expect(pale).not.toBeNull();
  });
  it('prefers a real colour to the fallback when the cover has one', () => {
    const data = pixels(...repeat([20, 20, 20, 255], 80), ...repeat([200, 40, 60, 255], 5));
    expect(dominantColor(data)).toBe('#c8283c');
  });
  it('returns null for empty input', () => {
    expect(dominantColor(new Uint8ClampedArray())).toBeNull();
  });
});

describe('ensureContrast', () => {
  it('lightens a dark colour until it reads on the dark theme', () => {
    const fixed = ensureContrast('#1a1a40', 'dark');
    expect(contrast(fixed, '#0f0f0f')).toBeGreaterThanOrEqual(3);
  });
  it('darkens a pale colour for the light theme', () => {
    const fixed = ensureContrast('#ffe9a0', 'light');
    expect(contrast(fixed, '#ffffff')).toBeGreaterThanOrEqual(3);
  });
  it('leaves an already readable colour alone', () => {
    expect(ensureContrast('#ff3b4a', 'dark')).toBe('#ff3b4a');
  });
});

describe('dominantPair', () => {
  it('gives the main colour and a second one of another hue', () => {
    const data = pixels(...repeat([200, 40, 60, 255], 60), ...repeat([40, 90, 200, 255], 30));
    const pair = dominantPair(data)!;
    expect(pair.a).toBe('#c8283c');
    expect(parseInt(pair.b.slice(5, 7), 16)).toBeGreaterThan(parseInt(pair.b.slice(1, 3), 16)); // bluish
  });
  it('makes a second colour of its own when the cover is all one colour', () => {
    const pair = dominantPair(pixels(...repeat([200, 40, 60, 255], 60)))!;
    expect(pair.a).toBe('#c8283c');
    expect(pair.b).not.toBe(pair.a);
  });
  it('has nothing for a transparent picture', () => {
    expect(dominantPair(pixels(...repeat([200, 40, 60, 0], 20)))).toBeNull();
  });
});

describe('cardTones and heroTones', () => {
  it('keep white words readable on a "Tune in" card whatever the cover', () => {
    for (const hex of ['#ffff00', '#ffffff', '#c8283c', '#14161e', '#40e0d0']) {
      const t = cardTones({ a: hex, b: hex });
      expect(contrast(t.a, '#ffffff'), hex).toBeGreaterThanOrEqual(4.5);
      expect(contrast(t.b, '#ffffff'), hex).toBeGreaterThanOrEqual(4.5);
    }
  });
  it('leave a grey cover grey', () => {
    const t = cardTones({ a: '#808080', b: '#404040' });
    expect(t.a.slice(1, 3)).toBe(t.a.slice(3, 5));
  });
  it('give the feature card three different colors', () => {
    const t = heroTones({ a: '#c8283c', b: '#2850c8' });
    expect(new Set([t.a, t.b, t.c]).size).toBe(3);
  });
});

/** The hue of a hex colour, in degrees. */
function hueOf(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255) as [number, number, number];
  const max = Math.max(r, g, b), d = max - Math.min(r, g, b);
  if (d === 0) return 0;
  const h = max === r ? ((g - b) / d + (g < b ? 6 : 0)) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return h * 60;
}
/** Green to the eye: a hue between yellow-green and cyan that has some colour in it. */
const isGreen = (hex: string) => {
  const ch = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  return hueOf(hex) > 75 && hueOf(hex) < 170 && Math.max(...ch) - Math.min(...ch) > 20;
};

describe('a yellow cover with white and black on it', () => {
  // Yellow, white and black type, a pale yellow: the sleeve that came out green.
  const cover = pixels(...repeat([252, 252, 30, 255], 50), ...repeat([255, 255, 255, 255], 25), ...repeat([10, 10, 10, 255], 15), ...repeat([245, 245, 205, 255], 10));
  it('is yellow with the black it has, not a made-up second colour', () => {
    const pair = dominantPair(cover)!;
    expect(pair.a).toBe('#fcfc1e');
    expect(contrast(pair.b, '#000000')).toBeLessThan(1.5); // near-black
  });
  it('reads black type that came out dark grey when the picture was made small', () => {
    const small = pixels(...repeat([245, 245, 245, 255], 55), ...repeat([250, 250, 30, 255], 30), ...repeat([82, 82, 82, 255], 4), ...repeat([214, 214, 214, 255], 11));
    const pair = dominantPair(small)!;
    expect(pair.a).toBe('#fafa1e');
    expect(pair.b).toBe('#525252');
  });
  it('gives no card, feature card or theme colour that is green', () => {
    const pair = dominantPair(cover)!;
    const card = cardTones(pair);
    const hero = heroTones(pair);
    const light = paletteFromArt(pair.a, 'light');
    const dark = paletteFromArt(pair.a, 'dark');
    for (const hex of [card.a, card.b, hero.a, hero.b, hero.c, light.accent, light.bg, light.surface, dark.accent, dark.bg, dark.surface]) {
      expect(isGreen(hex), hex).toBe(false);
    }
  });
  it('a cover of one colour only gets a darker one of the same hue, not one turned around the wheel', () => {
    const pair = dominantPair(pixels(...repeat([252, 252, 30, 255], 60)))!;
    expect(Math.abs(hueOf(pair.b) - hueOf(pair.a))).toBeLessThan(2);
    expect(heroTones(pair).c).not.toBe(heroTones(pair).a);
    expect(isGreen(heroTones(pair).c)).toBe(false);
  });
  it('gives a third colour when the cover has a third hue', () => {
    const pair = dominantPair(pixels(...repeat([200, 40, 60, 255], 50), ...repeat([40, 90, 200, 255], 30), ...repeat([240, 200, 20, 255], 20)))!;
    expect(pair.c).toBeDefined();
    expect(new Set([pair.a, pair.b, pair.c]).size).toBe(3);
  });
});

describe('deepHue', () => {
  it('turns yellow toward amber as it gets darker, and leaves light yellow and other hues alone', () => {
    expect(deepHue(60, 0.6)).toBe(60);
    expect(deepHue(60, 0.3)).toBeLessThan(48);
    expect(deepHue(60, 0.3)).toBeGreaterThan(40);
    expect(deepHue(20, 0.2)).toBe(20);
    expect(deepHue(120, 0.2)).toBe(120);
    expect(deepHue(200, 0.2)).toBe(200);
  });
});

describe('a grey cover', () => {
  const grey = pixels(...repeat([177, 177, 177, 255], 50), ...repeat([206, 204, 203, 255], 20), ...repeat([45, 45, 44, 255], 15), ...repeat([244, 243, 243, 255], 15));
  // How far from grey a colour is: the spread of its channels.
  const spread = (hex: string) => {
    const ch = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
    return Math.max(...ch) - Math.min(...ch);
  };
  it('gives a grey theme, not a red or orange one (grey has no hue to keep)', () => {
    const art = dominantColor(grey)!;
    for (const scheme of ['dark', 'light'] as const) {
      const p = paletteFromArt(art, scheme);
      for (const hex of [p.accent, p.bg, p.surface, p.surface2, p.line, p.fg, p.muted]) expect(spread(hex), `${scheme} ${hex}`).toBeLessThan(30);
    }
  });
  it('a faint tint still gives only a faint accent, a clear colour a strong one', () => {
    expect(spread(paletteFromArt('#b8aeaa', 'dark').accent)).toBeLessThan(spread(paletteFromArt('#d04030', 'dark').accent));
    expect(spread(paletteFromArt('#d04030', 'dark').accent)).toBeGreaterThan(80);
  });
});

describe('a cover without enough colour to have a hue', () => {
  const spread = (hex: string) => {
    const ch = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
    return Math.max(...ch) - Math.min(...ch);
  };
  it('stays neutral when only a faint cast or a handful of pixels of it are there (the grey cover that went pink)', () => {
    // Grey, black and white with 6 % of a faint pink cast: the old scoring picked the cast and made the theme salmon.
    const cover = pixels(...repeat([177, 177, 177, 255], 45), ...repeat([245, 245, 245, 255], 20), ...repeat([40, 40, 40, 255], 20), ...repeat([236, 215, 212, 255], 6), ...repeat([206, 204, 203, 255], 9));
    const pair = dominantPair(cover)!;
    expect(spread(pair.a)).toBe(0);
    expect(spread(pair.b)).toBeLessThan(10);
    expect(pair.c).toBeUndefined();
    expect(spread(paletteFromArt(dominantColor(cover)!, 'dark').accent)).toBeLessThan(10);
  });
  it('still takes a clear colour from a small area: a red logo on a black cover', () => {
    const cover = pixels(...repeat([20, 20, 20, 255], 80), ...repeat([200, 40, 60, 255], 6));
    expect(dominantColor(cover)).toBe('#c8283c');
  });
  it('picks the colour with the most colour in it, not the lightest or the most numerous faint one', () => {
    // A lot of faint beige, a smaller area of a clear blue: the blue is the cover's colour.
    const cover = pixels(...repeat([225, 215, 195, 255], 50), ...repeat([40, 90, 200, 255], 25), ...repeat([30, 30, 30, 255], 25));
    const [r, , b] = [1, 3, 5].map((i) => parseInt(dominantColor(cover)!.slice(i, i + 2), 16)) as [number, number, number];
    expect(b).toBeGreaterThan(r + 60);
  });
  it('gets a hue from each of up to three colours, 45 degrees or more apart', () => {
    const cover = pixels(...repeat([200, 40, 60, 255], 40), ...repeat([205, 60, 70, 255], 20), ...repeat([40, 90, 200, 255], 25), ...repeat([30, 160, 80, 255], 15));
    const pair = dominantPair(cover)!;
    expect(pair.c).toBeDefined();
    expect(new Set([pair.a, pair.b, pair.c]).size).toBe(3);
  });
});

describe('centreSquare', () => {
  it('is the whole picture when it is square', () => {
    expect(centreSquare(120, 120)).toEqual({ sx: 0, sy: 0, size: 120 });
  });
  it('cuts the bars off a 16:9 still, leaving the square cover art in its middle', () => {
    expect(centreSquare(1280, 720)).toEqual({ sx: 280, sy: 0, size: 720 });
    expect(centreSquare(320, 180)).toEqual({ sx: 70, sy: 0, size: 180 });
  });
  it('centres a tall picture too, and never returns an empty square', () => {
    expect(centreSquare(100, 300)).toEqual({ sx: 0, sy: 100, size: 100 });
    expect(centreSquare(0, 0).size).toBe(1);
  });
});
