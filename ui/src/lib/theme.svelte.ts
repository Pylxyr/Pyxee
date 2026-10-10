import { ensureContrast } from './color';

export type ThemeMode = 'system' | 'dark' | 'light';
export type Backdrop = 'mica' | 'none';
export interface ThemeSettings {
  mode: ThemeMode;
  accent: string;
  /** Take the accent from the current album art. */
  dynamicColor: boolean;
}

export const DEFAULT_ACCENT = '#ff3b4a';
const KEY = 'pyxee.theme';

export function resolveTheme(mode: ThemeMode, systemDark: boolean): 'dark' | 'light' {
  return mode === 'system' ? (systemDark ? 'dark' : 'light') : mode;
}

type Store = Pick<Storage, 'getItem' | 'setItem'>;

function load(storage: Store | null): ThemeSettings {
  const fallback: ThemeSettings = { mode: 'system', accent: DEFAULT_ACCENT, dynamicColor: false };
  try {
    const raw = storage?.getItem(KEY);
    if (!raw) return fallback;
    const v = JSON.parse(raw) as Partial<ThemeSettings>;
    return {
      mode: v.mode === 'dark' || v.mode === 'light' || v.mode === 'system' ? v.mode : fallback.mode,
      accent: typeof v.accent === 'string' && /^#[0-9a-f]{6}$/i.test(v.accent) ? v.accent : fallback.accent,
      dynamicColor: v.dynamicColor === true,
    };
  } catch {
    return fallback;
  }
}

export class ThemeStore {
  settings = $state<ThemeSettings>({ mode: 'system', accent: DEFAULT_ACCENT, dynamicColor: false });
  systemDark = $state(true);
  backdrop = $state<Backdrop>('none');
  /** Dominant colour of the current artwork, when known. */
  art = $state<string | null>(null);

  resolved = $derived(resolveTheme(this.settings.mode, this.systemDark));
  accent = $derived(
    this.settings.dynamicColor && this.art ? ensureContrast(this.art, this.resolved) : this.settings.accent,
  );

  #storage: Store | null;

  constructor(storage: Store | null = null, mq: MediaQueryList | null = null) {
    this.#storage = storage;
    this.settings = load(storage);
    if (mq) {
      this.systemDark = mq.matches;
      mq.addEventListener('change', (e) => (this.systemDark = e.matches));
    }
  }

  update(patch: Partial<ThemeSettings>) {
    this.settings = { ...this.settings, ...patch };
    try {
      this.#storage?.setItem(KEY, JSON.stringify(this.settings));
    } catch {
      /* storage unavailable: keep the in-memory value */
    }
  }

  /** Writes the resolved theme onto <html> so CSS variables take over. */
  apply(root: HTMLElement) {
    root.dataset.theme = this.resolved;
    root.dataset.backdrop = this.backdrop;
    root.style.setProperty('--accent', this.accent);
  }
}

export const theme = new ThemeStore(
  typeof localStorage !== 'undefined' ? localStorage : null,
  typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)') : null,
);
