import { DEFAULT_LOOK, lookAttributes, parseLook, type Look } from './look';

const KEY = 'pyxee.look';

function load(): Look {
  try {
    const raw = localStorage.getItem(KEY);
    return parseLook(raw ? JSON.parse(raw) : null);
  } catch {
    return { ...DEFAULT_LOOK };
  }
}

/** The look of the interface (Settings > Appearance). Saved in the UI's own storage, like the interface size. */
class LookStore {
  value = $state<Look>(load());

  set<K extends keyof Look>(key: K, v: Look[K]) {
    this.value = { ...this.value, [key]: v };
    try {
      localStorage.setItem(KEY, JSON.stringify(this.value));
    } catch {
      /* ignore */
    }
  }

  reset() {
    this.value = { ...DEFAULT_LOOK };
    try {
      localStorage.removeItem(KEY);
    } catch {
      /* ignore */
    }
  }

  /** Puts the look on the root element; returns what undoes it. Called from an effect in the app shell. */
  apply(root: HTMLElement = document.documentElement): void {
    for (const [name, v] of Object.entries(lookAttributes(this.value))) {
      if (v === null) root.removeAttribute(name);
      else root.setAttribute(name, v);
    }
  }
}

export const look = new LookStore();
