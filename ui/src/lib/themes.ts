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
  t('dark', 'Dark', 'dark', 'Neutral dark', '#0f0f0f #181818 #272727 #2a2a2a #ffffff #a8a8a8 #ff3b4a'),
  t('light', 'Light', 'light', 'Neutral light', '#ffffff #f4f4f4 #e8e8e8 #e0e0e0 #0f0f0f #5c5c5c #d92d3a'),
  t('ytmusic', 'YT Music', 'dark', 'Near-black with the red of YouTube Music', '#030303 #121212 #212121 #2f2f2f #ffffff #aaaaaa #ff0000'),
  t('oled', 'OLED black', 'dark', 'Pure black, easy on OLED screens', '#000000 #0a0a0a #1a1a1a #262626 #f2f2f2 #9a9a9a #ff5c6a'),
  t('deepblue', 'Deep blue', 'dark', 'Blue with violet highlights', '#0b1426 #111d36 #1b2a4a #243558 #e8eefc #9fb0d3 #8b7bff'),
  t('sky', 'Sky', 'light', 'Light blue with violet highlights', '#f3f8ff #e6f0fc #d5e5f8 #c3d7ee #14233b #50627f #6b4fd8'),
  t('graphitegold', 'Graphite and gold', 'dark', 'Warm grey with cream text and gold', '#1c1c1a #252522 #33332e #3d3d37 #f1e7c6 #aaa58d #d4a72c'),
  t('turquoise', 'Turquoise', 'dark', 'Dark slate with a turquoise player colour', '#101416 #181e21 #232b2f #2c363b #eef6f7 #9db1b6 #1fd1c1'),
  t('ember', 'Ember', 'dark', 'Dark brown with an orange glow', '#17110e #211813 #30221a #3b2a20 #fbeee6 #bfa392 #ff7a2f'),
  t('paper', 'Paper', 'light', 'Warm off-white with a burnt-orange accent', '#fffaf5 #f6ede4 #ebddd0 #e2d2c3 #2b1d14 #6b5648 #c2410c'),
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
