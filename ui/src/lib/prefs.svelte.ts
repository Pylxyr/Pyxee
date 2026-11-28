import { persist } from './persist';
import { isUiScale, type UiScale } from './uiscale';

const KEY = 'pyxee.volume';
const SCALE_KEY = 'pyxee.uiScale';
const FADE_KEY = 'pyxee.crossfade';

/** The crossfade lengths offered (seconds), 0 being off. The audio engine accepts 0 to 12. */
export const CROSSFADE_SECONDS = [0, 2, 3, 4, 5, 6, 8, 10, 12] as const;

function loadFade(): number {
  try {
    const v = Number(localStorage.getItem(FADE_KEY));
    return (CROSSFADE_SECONDS as readonly number[]).includes(v) && localStorage.getItem(FADE_KEY) !== null ? v : 0;
  } catch {
    return 0;
  }
}

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
  /** How many seconds one song fades into the next (0 = off, songs join without a gap). Pushed to the audio engine like the volume. */
  crossfade = $state(loadFade());
  setCrossfade(seconds: number) {
    const nearest = [...CROSSFADE_SECONDS].reduce((best, s) => (Math.abs(s - seconds) < Math.abs(best - seconds) ? s : best), 0);
    this.crossfade = Number.isFinite(seconds) ? nearest : 0;
    try {
      localStorage.setItem(FADE_KEY, String(this.crossfade));
      persist.touch();
    } catch {
      /* ignore */
    }
  }
  /** How big the interface is drawn (Settings -> Appearance). Applied by the app shell; only the real window can zoom. */
  uiScale = $state<UiScale>(loadScale());
  setUiScale(v: UiScale) {
    this.uiScale = v;
    try {
      localStorage.setItem(SCALE_KEY, v);
      persist.touch();
    } catch {
      /* ignore */
    }
  }
  setVolume(v: number) {
    this.volume = Math.max(0, Math.min(100, Math.round(v)));
    try {
      localStorage.setItem(KEY, String(this.volume));
      persist.touch();
    } catch {
      /* ignore */
    }
  }
}

export const prefs = new Prefs();
