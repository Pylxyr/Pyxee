/**
 * The choices under Settings > Appearance that change how the interface is drawn (not its colors: those are the
 * themes). Each one is a data attribute on the page's root element, so a choice is one CSS rule block in `app.css`,
 * never a script that runs. Kept apart from the store so it can be tested without a page.
 */

export interface LookOption<T extends string> {
  id: T;
  label: string;
  /** One line of what it looks like, for the settings page. */
  hint: string;
}

export const CORNERS = [
  { id: 'round', label: 'Round', hint: 'Pill buttons and tabs, big soft cards.' },
  { id: 'soft', label: 'Soft', hint: 'Gently rounded corners everywhere, no pills.' },
  { id: 'square', label: 'Square', hint: 'Sharp corners, like a spreadsheet.' },
] as const satisfies readonly LookOption<string>[];

export const SURFACES = [
  { id: 'vivid', label: 'Vivid', hint: 'Gradients, glows and soft shadows.' },
  { id: 'flat', label: 'Flat', hint: 'Plain colors and hairlines, no gradients or glows.' },
] as const satisfies readonly LookOption<string>[];

export const PANELS = [
  { id: 'floating', label: 'Floating', hint: 'In wide windows the bar, page, queue and player are separate cards with room between them.' },
  { id: 'docked', label: 'Docked', hint: 'In wide windows, one surface cut by thin lines, with no gaps.' },
] as const satisfies readonly LookOption<string>[];

export const COVERS = [
  { id: 'rounded', label: 'Rounded', hint: 'Covers follow the corner setting.' },
  { id: 'square', label: 'Square', hint: 'Covers are drawn like record sleeves, with sharp corners.' },
  { id: 'circle', label: 'Circle', hint: 'Covers are round.' },
] as const satisfies readonly LookOption<string>[];

export const HEROES = [
  { id: 'blur', label: 'Blur', hint: 'The cover, blurred into a wash behind the card and Now Playing.' },
  { id: 'aurora', label: 'Aurora', hint: 'Soft glows in the cover\'s own colors, on Home and Now Playing.' },
  { id: 'gradient', label: 'Gradient', hint: 'A clean sweep from one of the cover\'s colors to another, on Home and Now Playing.' },
  { id: 'vinyl', label: 'Vinyl', hint: 'A record slides out from behind the cover on Home; Now Playing gets a soft wash.' },
  { id: 'plain', label: 'Plain', hint: 'No background: just the card on Home, a plain Now Playing.' },
] as const satisfies readonly LookOption<string>[];

export const TUNES = [
  { id: 'cover', label: 'Album colors', hint: 'Each card takes the colors of its song\'s cover.' },
  { id: 'theme', label: 'Theme color', hint: 'Every card uses the theme\'s colors.' },
] as const satisfies readonly LookOption<string>[];

export const SEEK_CHOICES = [
  { id: 'glide', label: 'Smooth glide', hint: 'The bar slides continuously.' },
  { id: 'steps', label: 'Steps', hint: 'The bar moves in small steps, a few times a second.' },
  { id: 'step', label: 'Once a second', hint: 'The bar moves with the clock. Costs the least.' },
] as const satisfies readonly LookOption<string>[];

export type Corners = (typeof CORNERS)[number]['id'];
export type Surfaces = (typeof SURFACES)[number]['id'];
export type Panels = (typeof PANELS)[number]['id'];
export type Covers = (typeof COVERS)[number]['id'];
export type Hero = (typeof HEROES)[number]['id'];
export type Tune = (typeof TUNES)[number]['id'];

export interface Look {
  corners: Corners;
  surfaces: Surfaces;
  panels: Panels;
  covers: Covers;
  hero: Hero;
  tune: Tune;
}

export const DEFAULT_LOOK: Look = { corners: 'round', surfaces: 'vivid', panels: 'floating', covers: 'rounded', hero: 'blur', tune: 'cover' };

const CHOICES: { [K in keyof Look]: readonly { id: Look[K] }[] } = {
  corners: CORNERS,
  surfaces: SURFACES,
  panels: PANELS,
  covers: COVERS,
  hero: HEROES,
  tune: TUNES,
};

/** What was saved, kept to the choices that exist: anything unknown (a newer or damaged file) falls back to the default. */
export function parseLook(raw: unknown): Look {
  const out: Look = { ...DEFAULT_LOOK };
  if (typeof raw !== 'object' || raw === null) return out;
  const src = raw as Record<string, unknown>;
  for (const key of Object.keys(CHOICES) as (keyof Look)[]) {
    const v = src[key];
    if (CHOICES[key].some((c) => c.id === v)) (out[key] as string) = v as string;
  }
  return out;
}

/** The root element's attributes for a look. Names are `data-<key>`; the defaults are left off, so the base rules stay the default. */
export function lookAttributes(look: Look): Record<string, string | null> {
  const attrs: Record<string, string | null> = {};
  for (const key of Object.keys(CHOICES) as (keyof Look)[]) attrs[`data-${key}`] = look[key] === DEFAULT_LOOK[key] ? null : look[key];
  return attrs;
}
