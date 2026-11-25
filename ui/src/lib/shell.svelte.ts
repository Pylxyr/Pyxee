import type { Backend } from './backend';
import { messageOf } from './backend';
import type { AudioDevicesDto } from './bindings/AudioDevicesDto';
import type { ShellSettingsDto } from './bindings/ShellSettingsDto';

const OFF: ShellSettingsDto = { closeToTray: false, startWithWindows: false, toastOnChange: false, trayControls: false, flyoutOnClick: true, discordPresence: false, lastfmScrobbling: false, checkPrereleases: false };

/** Tray, start-with-Windows and toast settings. A change shows at once and goes back if it could not be saved. */
class ShellStore {
  settings = $state<ShellSettingsDto>({ ...OFF });
  /** Set when the last change could not be saved. */
  error = $state<string | null>(null);
  /** The output devices that are connected, and the one chosen; `null` until they have been asked for. */
  devices = $state<AudioDevicesDto | null>(null);
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

  /** Asks which output devices are connected. A failure leaves the control hidden: playing on the default still works. */
  async loadDevices() {
    if (!this.#backend) return;
    this.devices = await this.#backend.audioDevicesGet().catch(() => null);
  }

  /** Chooses the output device (`null`: the system's default). It goes back to what it was if the choice was refused. */
  async chooseDevice(id: string | null) {
    if (!this.#backend) return;
    this.error = null;
    try {
      this.devices = await this.#backend.audioDeviceSet(id);
    } catch (e) {
      this.error = messageOf(e);
      await this.loadDevices();
    }
  }

  /** Forgets the Last.fm sign-in and the scrobbles still waiting, and turns scrobbling off. Resolves to whether it worked. */
  async clearLastfm(): Promise<boolean> {
    if (!this.#backend) return false;
    this.error = null;
    try {
      this.settings = await this.#backend.lastfmClear();
      return true;
    } catch (e) {
      this.error = messageOf(e);
      return false;
    }
  }
}

export const shellSettings = new ShellStore();
