import { ensureContrastOn } from './color';
import {
  BUILTIN_THEMES, DEFAULT_DARK_ID, DEFAULT_LIGHT_ID, MAX_CUSTOM_THEMES, sanitizeTheme, themeVars,
  type ThemeDef,
} from './themes';

export type Backdrop = 'mica' | 'none';

export interface ThemeSettings {
  /** Use `lightId` or `darkId` according to the system's light/dark setting. */
  followSystem: boolean;
  /** The theme used when not following the system. */
  themeId: string;
  lightId: string;
  darkId: string;
  /** Take the accent from the current album art. */
  dynamicColor: boolean;
  /** Themes the user made or imported. */
  custom: ThemeDef[];
}

const KEY = 'pyxee.theme';
const BUILTIN_IDS = new Set(BUILTIN_THEMES.map((t) => t.id));

type Store = Pick<Storage, 'getItem' | 'setItem'>;

function defaults(): ThemeSettings {
  return { followSystem: true, themeId: DEFAULT_DARK_ID, lightId: DEFAULT_LIGHT_ID, darkId: DEFAULT_DARK_ID, dynamicColor: false, custom: [] };
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
  accent = $derived(
    this.settings.dynamicColor && this.art && !this.preview
      ? ensureContrastOn(this.art, this.active.colors.bg, this.active.scheme)
      : this.active.colors.accent,
  );
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

  #save() {
    try {
      this.#storage?.setItem(KEY, JSON.stringify(this.settings));
    } catch {
      /* storage unavailable: keep the in-memory value */
    }
  }

  update(patch: Partial<Omit<ThemeSettings, 'custom'>>) {
    this.settings = { ...this.settings, ...patch };
    this.#save();
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

  /** Writes the active theme onto <html> so the CSS variables take over. */
  apply(root: HTMLElement) {
    const def = this.active;
    root.dataset.theme = def.scheme;
    root.dataset.themeId = def.id || 'preview';
    root.dataset.backdrop = this.backdrop;
    const vars = themeVars({ ...def.colors, accent: this.accent });
    for (const [name, value] of Object.entries(vars)) root.style.setProperty(name, value);
  }
}

export const theme = new ThemeStore(
  typeof localStorage !== 'undefined' ? localStorage : null,
  typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)') : null,
);
