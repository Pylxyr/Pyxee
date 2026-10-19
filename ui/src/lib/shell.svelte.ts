import type { Backend } from './backend';
import { messageOf } from './backend';
import type { ShellSettingsDto } from './bindings/ShellSettingsDto';

const OFF: ShellSettingsDto = { closeToTray: false, startWithWindows: false, toastOnChange: false, trayControls: false, flyoutOnClick: true, discordPresence: true };

/** Tray, start-with-Windows and toast settings. A change shows at once and goes back if it could not be saved. */
class ShellStore {
  settings = $state<ShellSettingsDto>({ ...OFF });
  /** Set when the last change could not be saved. */
  error = $state<string | null>(null);
  #backend: Backend | null = null;

  async init(backend: Backend) {
    this.#backend = backend;
    this.settings = await backend.shellGet().catch(() => ({ ...OFF }));
  }

  async update(patch: Partial<ShellSettingsDto>) {
    if (!this.#backend) return;
    const before = this.settings;
    this.settings = { ...before, ...patch };
    this.error = null;
    try {
      this.settings = await this.#backend.shellSet(this.settings);
    } catch (e) {
      this.settings = before;
      this.error = messageOf(e);
    }
  }
}

export const shellSettings = new ShellStore();
