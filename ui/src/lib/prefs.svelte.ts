import { isUiScale, type UiScale } from './uiscale';

const KEY = 'pyxee.volume';
const SCALE_KEY = 'pyxee.uiScale';

function loadScale(): UiScale {
  try {
    const v = localStorage.getItem(SCALE_KEY);
    return isUiScale(v) ? v : 'auto';
  } catch {
    return 'auto';
  }
}

function load(): number {
  try {
    const v = Number(localStorage.getItem(KEY));
    return Number.isFinite(v) && v >= 0 && v <= 100 && localStorage.getItem(KEY) !== null ? v : 80;
  } catch {
    return 80;
  }
}

/** Volume (0-100). Stored in the UI for now; the audio engine consumes it once real playback lands (milestone 4). */
class Prefs {
  volume = $state(load());
  /** How big the interface is drawn (Settings -> Appearance). Applied by the app shell; only the real window can zoom. */
  uiScale = $state<UiScale>(loadScale());
  setUiScale(v: UiScale) {
    this.uiScale = v;
    try {
      localStorage.setItem(SCALE_KEY, v);
    } catch {
      /* ignore */
    }
  }
  setVolume(v: number) {
    this.volume = Math.max(0, Math.min(100, Math.round(v)));
    try {
      localStorage.setItem(KEY, String(this.volume));
    } catch {
      /* ignore */
    }
  }
}

export const prefs = new Prefs();
