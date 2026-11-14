import type { Backend } from './backend';
import type { SleepSettingDto } from './bindings/SleepSettingDto';
import type { SleepStatusDto } from './bindings/SleepStatusDto';

class SleepStore {
  status = $state<SleepStatusDto | null>(null);
  #backend: Backend | null = null;
  /** The window may have been asleep (hidden in the tray) while pushes were skipped: ask again when it wakes. */
  #wake = () => {
    if (!document.hidden) void this.resync();
  };

  async init(backend: Backend) {
    this.#backend = backend;
    await backend.onSleep((s) => (this.status = s.status));
    if (typeof document !== 'undefined') {
      document.removeEventListener('visibilitychange', this.#wake);
      document.addEventListener('visibilitychange', this.#wake);
    }
    this.status = (await backend.sleepGet()).status;
  }

  /** Takes the timer state again (what is shown stays if it cannot be read). */
  async resync() {
    try {
      const s = await this.#backend?.sleepGet();
      if (s) this.status = s.status;
    } catch {
      /* keep what is shown; the next push replaces it */
    }
  }

  set = (setting: SleepSettingDto) => this.#backend?.sleepSet(setting);
  cancel = () => this.#backend?.sleepCancel();
  extend = (minutes: number) => this.#backend?.sleepExtend(minutes);
}

export const sleep = new SleepStore();
