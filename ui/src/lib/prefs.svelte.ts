const KEY = 'pyxee.volume';

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
