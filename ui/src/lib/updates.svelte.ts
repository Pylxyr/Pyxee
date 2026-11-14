import { messageOf, type Backend } from './backend';
import type { AppUpdateDto } from './bindings/AppUpdateDto';
import type { UpdateProgressDto } from './bindings/UpdateProgressDto';

export type UpdatePhase = 'idle' | 'checking' | 'installing';

/**
 * The state of the Pyxee update itself, shared by the Settings sheet and the changelog panel.
 * A check that finds a newer version opens the changelog at once, whether the person asked for the check
 * (Settings > Updates) or Pyxee did it by itself at start-up.
 */
class UpdateStore {
  phase = $state<UpdatePhase>('idle');
  info = $state<AppUpdateDto | null>(null);
  /** Why the last check or install failed. A failed check at start-up stays silent (offline is normal). */
  error = $state<string | null>(null);
  progress = $state<UpdateProgressDto | null>(null);
  changelogOpen = $state(false);

  #off: (() => void) | null = null;

  /** Follows the installer download. Called once, when the backend is ready. */
  async init(backend: Backend) {
    this.#off?.();
    this.#off = await backend.onUpdateProgress((p) => {
      if (p.target === 'app') this.progress = p;
    });
  }

  /** The check Pyxee runs by itself when it starts. Silent unless a newer version exists. */
  checkAtStartup(backend: Backend) {
    return this.#check(backend, false);
  }

  /** The check behind the "Check for updates" button: failures are shown. */
  check(backend: Backend) {
    return this.#check(backend, true);
  }

  async #check(backend: Backend, report: boolean) {
    if (this.phase !== 'idle') return;
    this.phase = 'checking';
    if (report) this.error = null;
    try {
      this.info = await backend.updateCheckApp();
      if (this.info.available) this.changelogOpen = true;
    } catch (e) {
      if (report) {
        this.info = null;
        this.error = messageOf(e);
      }
    }
    this.phase = 'idle';
  }

  async install(backend: Backend) {
    this.phase = 'installing';
    this.error = null;
    this.progress = null;
    try {
      await backend.updateInstallApp(); // starts the installer and closes Pyxee
    } catch (e) {
      this.error = messageOf(e);
    }
    this.phase = 'idle';
    this.progress = null;
  }

  openChangelog() {
    this.changelogOpen = true;
  }
  closeChangelog() {
    this.changelogOpen = false;
  }

  /** Back to the start (tests). */
  reset() {
    this.#off?.();
    this.#off = null;
    this.phase = 'idle';
    this.info = null;
    this.error = null;
    this.progress = null;
    this.changelogOpen = false;
  }
}

export const updates = new UpdateStore();
