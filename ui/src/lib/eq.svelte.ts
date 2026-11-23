import type { Backend } from './backend';
import { messageOf } from './backend';
import type { EqDto } from './bindings/EqDto';
import { FLAT_EQ, clampTenths } from './eq';

/**
 * The equalizer. A change shows at once and goes to the audio engine at once; `save: false` is for the middle of a drag
 * (the engine hears every step, the settings file is written once, when the slider is let go).
 */
class EqStore {
  settings = $state<EqDto>({ ...FLAT_EQ, gains: [...FLAT_EQ.gains] });
  /** Set when the last change could not be applied. */
  error = $state<string | null>(null);
  #backend: Backend | null = null;

  async init(backend: Backend) {
    this.#backend = backend;
    const saved = await backend.eqGet().catch(() => null);
    if (saved) this.settings = saved;
  }

  async update(patch: Partial<EqDto>, save: boolean) {
    const next: EqDto = { ...this.settings, ...patch };
    next.preamp = clampTenths(next.preamp);
    next.gains = next.gains.map(clampTenths);
    this.settings = next;
    if (!this.#backend) return;
    try {
      await this.#backend.eqSet(next, save);
      this.error = null;
    } catch (e) {
      this.error = messageOf(e);
    }
  }

  setGain(band: number, tenths: number, save: boolean) {
    const gains = [...this.settings.gains];
    gains[band] = tenths;
    return this.update({ gains }, save);
  }
}

export const eq = new EqStore();
