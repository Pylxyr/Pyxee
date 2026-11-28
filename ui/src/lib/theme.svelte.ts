import { persist } from './persist';
import type { Backend } from './backend';
import { coverRolesFromImage, paletteFromArt, paletteFromRoles, windowTint, type CoverRoles, type RolePalette, type WindowTint } from './color';
import { crossfadeTheme } from './motion';
import {
  BUILTIN_THEMES, DEFAULT_DARK_ID, DEFAULT_LIGHT_ID, MAX_CUSTOM_THEMES, sanitizeTheme, themeVars,
  type ThemeColors, type ThemeDef,
} from './themes';

export type Backdrop = 'mica' | 'none';

/** What "Color from album art" takes from a cover: its one main colour, or all of its colours spread over the interface. */
export type DynamicMode = 'single' | 'several';

export interface ThemeSettings {
  /** Use `lightId` or `darkId` according to the system's light/dark setting. */
  followSystem: boolean;
  /** The theme used when not following the system. */
  themeId: string;
  lightId: string;
  darkId: string;
  /** Take the accent from the current album art. */
  dynamicColor: boolean;
  /** With `dynamicColor` on: one colour from the cover (the accent, the rest turned from it) or several (see `paletteFromRoles`). */
  dynamicMode: DynamicMode;
  /** Themes the user made or imported. */
  custom: ThemeDef[];
}

const KEY = 'pyxee.theme';
const BUILTIN_IDS = new Set(BUILTIN_THEMES.map((t) => t.id));

type Store = Pick<Storage, 'getItem' | 'setItem'>;

function defaults(): ThemeSettings {
  return { followSystem: true, themeId: DEFAULT_DARK_ID, lightId: DEFAULT_LIGHT_ID, darkId: DEFAULT_DARK_ID, dynamicColor: false, dynamicMode: 'several', custom: [] };
}

function load(storage: Store | null): ThemeSettings {
  const fallback = defaults();
  try {
    const raw = storage?.getItem(KEY);
    if (!raw) return fallback;
    const v = JSON.parse(raw) as Record<string, unknown>;
    const custom: ThemeDef[] = [];
    if (Array.isArray(v.custom)) {
      for (const c of v.custom) {
        const def = sanitizeTheme(c);
        if (def && def.id && !BUILTIN_IDS.has(def.id) && !custom.some((x) => x.id === def.id) && custom.length < MAX_CUSTOM_THEMES) custom.push(def);
      }
    }
    const all = [...BUILTIN_THEMES, ...custom];
    const pick = (id: unknown, scheme: 'dark' | 'light' | null, other: string) => {
      const found = typeof id === 'string' ? all.find((t) => t.id === id) : undefined;
      return found && (scheme === null || found.scheme === scheme) ? found.id : other;
    };
    // Settings from before themes existed: { mode: 'system' | 'dark' | 'light' }.
    const legacy = v.mode === 'dark' || v.mode === 'light';
    return {
      followSystem: legacy ? false : v.followSystem !== false,
      themeId: legacy ? (v.mode as string) : pick(v.themeId, null, DEFAULT_DARK_ID),
      lightId: pick(v.lightId, 'light', DEFAULT_LIGHT_ID),
      darkId: pick(v.darkId, 'dark', DEFAULT_DARK_ID),
      dynamicColor: v.dynamicColor === true,
      // Someone who already has the colour on keeps the one-colour look until they choose; everyone else starts with several.
      dynamicMode: v.dynamicMode === 'single' || v.dynamicMode === 'several' ? v.dynamicMode : v.dynamicColor === true ? 'single' : 'several',
      custom,
    };
  } catch {
    return fallback;
  }
}

export class ThemeStore {
  settings = $state<ThemeSettings>(defaults());
  systemDark = $state(true);
  backdrop = $state<Backdrop>('none');
  /** Dominant colour of the current artwork, when known. */
  art = $state<string | null>(null);
  /** What the current artwork is made of (its colours and neutrals), when known; `art` is its primary. */
  roles = $state<CoverRoles | null>(null);
  /** A theme being edited: shown live, not yet saved. */
  preview = $state<ThemeDef | null>(null);

  all = $derived<ThemeDef[]>([...BUILTIN_THEMES, ...this.settings.custom]);
  /** Id of the theme the settings point at (ignoring any preview). */
  selectedId = $derived(
    this.settings.followSystem ? (this.systemDark ? this.settings.darkId : this.settings.lightId) : this.settings.themeId,
  );
  active = $derived<ThemeDef>(
    this.preview ?? this.all.find((t) => t.id === this.selectedId) ?? BUILTIN_THEMES[0]!,
  );
  resolved = $derived(this.active.scheme);
  /** The same, with the cover's other colours spread over the interface (`accent2`, `accent3`), when the cover colours the app. */
  rolePalette = $derived<RolePalette | null>(
    this.settings.dynamicColor && this.roles && !this.preview ? paletteFromRoles(this.roles, this.active.scheme) : null,
  );
  /** The cover's roles worn as the palette, when the choice is "Several colours" and a cover is known; else null. */
  spread = $derived<RolePalette | null>(this.settings.dynamicMode === 'several' ? this.rolePalette : null);
  /**
   * The colours in use: the active theme's, or, with "Color from album art" on and a cover to take it from, a whole
   * palette made from the cover's colour (the theme's light or dark stays).
   */
  colors = $derived<ThemeColors>(
    this.spread
      ? this.spread.colors
      : this.settings.dynamicColor && this.art && !this.preview ? paletteFromArt(this.art, this.active.scheme) : this.active.colors,
  );
  accent = $derived(this.colors.accent);
  /** What the native window frame and backdrop should follow: the system (null) or a fixed light/dark. */
  windowTheme = $derived<'dark' | 'light' | null>(this.settings.followSystem && !this.preview ? null : this.resolved);

  #storage: Store | null;

  constructor(storage: Store | null = null, mq: MediaQueryList | null = null) {
    this.#storage = storage;
    this.settings = load(storage);
    if (mq) {
      this.systemDark = mq.matches;
      mq.addEventListener('change', (e) => (this.systemDark = e.matches));
    }
  }

  /** Takes up settings another window saved. */
  reload() {
    this.settings = load(this.#storage);
  }

  #save() {
    try {
      this.#storage?.setItem(KEY, JSON.stringify(this.settings));
      persist.touch();
    } catch {
      /* storage unavailable: keep the in-memory value */
    }
  }

  /**
   * Changes settings. A change the person can SEE as another palette (a theme, following the system, the cover's colour)
   * crossfades; with view transitions the new settings are then applied inside the transition, a moment after this returns.
   */
  update(patch: Partial<Omit<ThemeSettings, 'custom'>>) {
    const apply = () => {
      this.settings = { ...this.settings, ...patch };
      this.#save();
    };
    const looks = (['followSystem', 'themeId', 'lightId', 'darkId', 'dynamicColor', 'dynamicMode'] as const).some((k) => k in patch && patch[k] !== this.settings[k]);
    if (looks) crossfadeTheme(apply);
    else apply();
  }

  /** Picks one theme for good (and stops following the system). */
  select(id: string) {
    if (this.all.some((t) => t.id === id)) this.update({ followSystem: false, themeId: id });
  }

  find(id: string): ThemeDef | undefined {
    return this.all.find((t) => t.id === id);
  }

  /** Saves a custom theme: a new one when `def.id` is empty or unknown, else replaces it. Returns what was stored. */
  saveCustom(def: ThemeDef): ThemeDef {
    const existing = def.id && this.settings.custom.some((t) => t.id === def.id);
    if (!existing && this.settings.custom.length >= MAX_CUSTOM_THEMES) {
      throw new Error(`You can keep up to ${MAX_CUSTOM_THEMES} custom themes. Delete one first.`);
    }
    const saved: ThemeDef = { id: existing ? def.id : this.#newId(), name: def.name.trim() || 'My theme', scheme: def.scheme, colors: { ...def.colors } };
    const custom = existing ? this.settings.custom.map((t) => (t.id === saved.id ? saved : t)) : [...this.settings.custom, saved];
    const all = [...BUILTIN_THEMES, ...custom];
    // A theme switched from dark to light can no longer serve as the system-dark theme (and vice versa).
    const ok = (id: string, scheme: 'dark' | 'light') => all.find((t) => t.id === id)?.scheme === scheme;
    this.settings = {
      ...this.settings,
      custom,
      darkId: ok(this.settings.darkId, 'dark') ? this.settings.darkId : DEFAULT_DARK_ID,
      lightId: ok(this.settings.lightId, 'light') ? this.settings.lightId : DEFAULT_LIGHT_ID,
    };
    this.#save();
    return saved;
  }

  deleteCustom(id: string) {
    if (!this.settings.custom.some((t) => t.id === id)) return;
    this.settings = {
      ...this.settings,
      custom: this.settings.custom.filter((t) => t.id !== id),
      themeId: this.settings.themeId === id ? DEFAULT_DARK_ID : this.settings.themeId,
      darkId: this.settings.darkId === id ? DEFAULT_DARK_ID : this.settings.darkId,
      lightId: this.settings.lightId === id ? DEFAULT_LIGHT_ID : this.settings.lightId,
    };
    this.#save();
  }

  #newId(): string {
    let id: string;
    do id = `custom-${Date.now().toString(36)}${Math.floor(Math.random() * 1296).toString(36)}`;
    while (this.all.some((t) => t.id === id));
    return id;
  }

  /** The colours for the native window frame, while the palette comes from the cover; `null` leaves the system's. */
  nativeTint = $derived<WindowTint | null>(
    this.settings.dynamicColor && this.art && !this.preview ? windowTint(this.colors) : null,
  );

  /** Writes the active theme onto <html> so the CSS variables take over. */
  apply(root: HTMLElement) {
    const def = this.active;
    root.dataset.theme = def.scheme;
    root.dataset.themeId = def.id || 'preview';
    root.dataset.backdrop = this.backdrop;
    const vars = themeVars(this.colors, this.spread ?? undefined);
    for (const [name, value] of Object.entries(vars)) root.style.setProperty(name, value);
    // Without several colours the stylesheet's own second colour (turned from the accent) must show again.
    if (!this.spread) for (const name of ['--t-accent-2', '--t-accent-3', '--t-on-accent-2']) root.style.removeProperty(name);
  }
}

export const theme = new ThemeStore(
  typeof localStorage !== 'undefined' ? localStorage : null,
  typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)') : null,
);

/**
 * Keeps `theme.art` in step with the cover of the song that is playing, for "Color from album art". Call it while a
 * component is being set up: the main window and the small windows (flyout, mini player) all do, so all of them wear the
 * same palette.
 */
/** Waits before asking for a cover's colour again after a miss: a picture that did not load a moment ago often does now. */
export const ART_RETRY_MS = [1500, 4000];

export function followArt(thumbnail: () => string | null, backend: () => Backend | null): void {
  $effect(() => {
    const url = thumbnail();
    if (!theme.settings.dynamicColor || !url) {
      theme.art = null;
      theme.roles = null;
      return;
    }
    let live = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const b = backend();
    const fetchBytes = b ? (u: string) => b.coverBytes(u) : undefined;
    const attempt = (n: number) => {
      void coverRolesFromImage(url, fetchBytes).then((c) => {
        if (!live) return;
        if (c) {
          theme.art = c.primary;
          theme.roles = c;
        } else if (n < ART_RETRY_MS.length) {
          timer = setTimeout(() => attempt(n + 1), ART_RETRY_MS[n]);
        } else {
          theme.art = null;
          theme.roles = null;
        }
      });
    };
    attempt(0);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  });
}
