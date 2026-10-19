import { HEX, contrast, normalizeHex, readableOn } from './color';

/** How far apart the strongest and weakest channel are (0 for greys; 8 or more is visibly tinted, even when dark). */
function chroma(hex: string): number {
  const n = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  return Math.max(...n) - Math.min(...n);
}

export type Scheme = 'dark' | 'light';

/** The seven colours a theme is made of. Everything else (button text, overlays) is derived from them. */
export interface ThemeColors {
  /** Page background. */
  bg: string;
  /** Panels and cards. */
  surface: string;
  /** Raised or selected things, menus, slider tracks. */
  surface2: string;
  /** Dividers. */
  line: string;
  /** Main text. */
  fg: string;
  /** Secondary text and icons. */
  muted: string;
  /** Highlights: seek bar, liked heart, focus ring. */
  accent: string;
}

export type ColorKey = keyof ThemeColors;

export interface ThemeDef {
  id: string;
  name: string;
  /** Whether it is a dark or a light theme: decides form controls, scroll bars and the window frame. */
  scheme: Scheme;
  colors: ThemeColors;
  description?: string;
  builtin?: boolean;
}

export const COLOR_FIELDS: { key: ColorKey; label: string; hint: string }[] = [
  { key: 'bg', label: 'Background', hint: 'The page behind everything' },
  { key: 'surface', label: 'Panels', hint: 'Cards, the player bar, hovered rows' },
  { key: 'surface2', label: 'Raised', hint: 'Menus, selected rows, slider tracks' },
  { key: 'line', label: 'Dividers', hint: 'Thin lines between areas' },
  { key: 'fg', label: 'Text', hint: 'Titles and body text' },
  { key: 'muted', label: 'Secondary text', hint: 'Artists, durations, icons' },
  { key: 'accent', label: 'Accent', hint: 'Seek bar, likes, focus ring' },
];

const t = (id: string, name: string, scheme: Scheme, description: string, c: string): ThemeDef => {
  const [bg, surface, surface2, line, fg, muted, accent] = c.split(' ') as [string, string, string, string, string, string, string];
  return { id, name, scheme, description, builtin: true, colors: { bg, surface, surface2, line, fg, muted, accent } };
};

/**
 * Built in. The last few are original palettes in the spirit of the colour schemes the MusicBee skin community
 * makes (blue with violet highlights, grey with gold, a dark base with a bright "player colour"); they are not
 * copies of any particular skin.
 */
export const BUILTIN_THEMES: ThemeDef[] = [
  t('dark', 'Ink', 'dark', 'Blue-black ink with an amber accent (the default)', '#12141b #1a1d27 #252a37 #2e3444 #eef0f6 #9ba3b7 #f2a93b'),
  t('light', 'Frost', 'light', 'Cool off-white with a burnt-amber accent', '#f4f6fa #eaeef5 #dde3ee #cdd5e3 #141824 #566078 #b45309'),
  t('oled', 'OLED black', 'dark', 'Pure black, easy on OLED screens', '#000000 #0a0a0a #1a1a1a #262626 #f2f2f2 #9a9a9a #ff5c6a'),
  t('deepblue', 'Deep blue', 'dark', 'Blue with violet highlights', '#0b1426 #111d36 #1b2a4a #243558 #e8eefc #9fb0d3 #8b7bff'),
  t('sky', 'Sky', 'light', 'Light blue with violet highlights', '#f3f8ff #e6f0fc #d5e5f8 #c3d7ee #14233b #50627f #6b4fd8'),
  t('graphitegold', 'Graphite and gold', 'dark', 'Warm grey with cream text and gold', '#1c1c1a #252522 #33332e #3d3d37 #f1e7c6 #aaa58d #d4a72c'),
  t('turquoise', 'Turquoise', 'dark', 'Dark slate with a turquoise player colour', '#101416 #181e21 #232b2f #2c363b #eef6f7 #9db1b6 #1fd1c1'),
  t('ember', 'Ember', 'dark', 'Dark brown with an orange glow', '#17110e #211813 #30221a #3b2a20 #fbeee6 #bfa392 #ff7a2f'),
  t('paper', 'Paper', 'light', 'Warm off-white with a burnt-orange accent', '#fffaf5 #f6ede4 #ebddd0 #e2d2c3 #2b1d14 #6b5648 #c2410c'),
  // Dark
  t('amethyst', 'Amethyst', 'dark', 'Deep purple-black with a soft violet accent', '#120d1f #1a1429 #271e3d #33284d #f1ecfb #b0a3cc #b388ff'),
  t('forest', 'Forest', 'dark', 'Dark green-black with a fresh green accent', '#0d1410 #141d17 #1e2b22 #283a2d #e9f3ec #9bb4a3 #4ade80'),
  t('crimson', 'Crimson', 'dark', 'Dark wine with a crimson-pink accent', '#170c10 #211217 #301a22 #3d222c #fbeaee #c0a0a9 #ff4d6d'),
  t('synthwave', 'Synthwave', 'dark', 'Night purple with a hot pink accent', '#130a24 #1c1133 #2a1b4a #37245d #f6eefe #b9a2d6 #ff3ea5'),
  t('steel', 'Steel', 'dark', 'Cool blue-grey with a sky-blue accent', '#161a20 #1e242c #2a323d #343e4b #e8edf3 #9aa7b8 #5eb8ff'),
  t('midnightsun', 'Midnight sun', 'dark', 'Navy with a golden-yellow accent', '#0c1320 #131d2f #1d2a42 #26364f #eaf0fa #9aaccb #ffcf4a'),
  t('abyss', 'Abyss', 'dark', 'Deep sea green with a bright teal accent', '#071a1c #0d2427 #153238 #1d4046 #e6f6f6 #8fb6b9 #2dd4bf'),
  // Light
  t('mint', 'Mint', 'light', 'Pale green with a deep teal accent', '#f1faf5 #e3f3ea #d2e9dc #bfdcca #10241a #4a6556 #0f766e'),
  t('lavender', 'Lavender', 'light', 'Soft lilac with a rich purple accent', '#f7f4fd #ede7f9 #e0d6f3 #d0c3e8 #1d1530 #5d5177 #6d28d9'),
  t('rose', 'Rose', 'light', 'Blush pink with a raspberry accent', '#fff5f7 #fde8ed #f9d6df #efc3cf #2a1018 #6e4a56 #be185d'),
  t('sandstone', 'Sandstone', 'light', 'Warm sand with a bronze accent', '#f8f4ec #efe8da #e3d9c5 #d6cab2 #2a2417 #645a44 #8a5a00'),
  t('silver', 'Silver', 'light', 'Neutral light grey with a clear blue accent', '#f4f5f7 #e9ebee #dcdfe4 #cbd0d7 #16181d #565c68 #2563eb'),
  t('lemonade', 'Lemonade', 'light', 'Pale lemon with a dark gold accent', '#fffdf0 #f7f3d9 #ede7bf #e0d9a6 #26230d #615c33 #926200'),
];

export const DEFAULT_DARK_ID = 'dark';
export const DEFAULT_LIGHT_ID = 'light';
export const MAX_CUSTOM_THEMES = 30;

/** The CSS variables `app.css` reads (`--t-*`), derived from a theme's seven colours. */
export function themeVars(colors: ThemeColors): Record<string, string> {
  return {
    '--t-bg': colors.bg,
    '--t-surface': colors.surface,
    '--t-surface-2': colors.surface2,
    '--t-line': colors.line,
    '--t-fg': colors.fg,
    '--t-muted': colors.muted,
    '--t-accent': colors.accent,
    // Opaque twins that stay solid on Windows 11 Mica (which makes the page background transparent).
    '--t-solid': colors.bg,
    '--t-solid-2': colors.surface2,
    // Text on a button filled with the text colour, and on one filled with the accent.
    '--t-on-fg': readableOn(colors.fg),
    '--t-on-accent': readableOn(colors.accent),
    // On Windows 11 Mica a grey background lets the backdrop show through fully; a coloured one tints it.
    '--t-mica-tint': chroma(colors.bg) < 8 ? '0%' : '70%',
  };
}

export interface ContrastCheck {
  label: string;
  ratio: number;
  min: number;
  ok: boolean;
}

/** The pairings that decide whether a theme is readable (WCAG contrast ratios). */
export function contrastChecks(c: ThemeColors): ContrastCheck[] {
  const check = (label: string, a: string, b: string, min: number): ContrastCheck => {
    const ratio = contrast(a, b);
    return { label, ratio, min, ok: ratio >= min };
  };
  return [
    check('Text on background', c.fg, c.bg, 4.5),
    check('Text on panels', c.fg, c.surface, 4.5),
    check('Secondary text on background', c.muted, c.bg, 4.5),
    check('Secondary text on raised', c.muted, c.surface2, 3),
    check('Accent on background', c.accent, c.bg, 3),
  ];
}

const COLOR_KEYS = COLOR_FIELDS.map((f) => f.key);

/** A theme from untrusted data (saved settings, pasted text); null if it is not usable. */
export function sanitizeTheme(raw: unknown): ThemeDef | null {
  if (typeof raw !== 'object' || raw === null) return null;
  const r = raw as Record<string, unknown>;
  const name = typeof r.name === 'string' ? r.name.trim().slice(0, 40) : '';
  const scheme = r.scheme === 'light' || r.scheme === 'dark' ? r.scheme : null;
  const cr = r.colors;
  if (!name || !scheme || typeof cr !== 'object' || cr === null) return null;
  const colors = {} as ThemeColors;
  for (const k of COLOR_KEYS) {
    const v = (cr as Record<string, unknown>)[k];
    const hex = typeof v === 'string' ? normalizeHex(v) : null;
    if (!hex) return null;
    colors[k] = hex;
  }
  const id = typeof r.id === 'string' && /^[a-z0-9-]{1,40}$/.test(r.id) ? r.id : '';
  return { id, name, scheme, colors };
}

export function exportTheme(def: ThemeDef): string {
  return JSON.stringify({ pyxeeTheme: 1, name: def.name, scheme: def.scheme, colors: def.colors }, null, 2);
}

/** Reads text produced by `exportTheme`; throws a sentence the UI can show. */
export function parseThemeText(text: string): ThemeDef {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    throw new Error("That isn't theme text. Paste what Export copied.");
  }
  const def = sanitizeTheme(raw);
  if (!def) throw new Error('That theme is missing a name, a dark/light setting or one of its colours.');
  return { ...def, id: '' };
}

export function isHexColor(v: string): boolean {
  return HEX.test(v);
}
