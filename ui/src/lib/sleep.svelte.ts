import type { Backend } from './backend';
import type { SleepSettingDto } from './bindings/SleepSettingDto';
import type { SleepStatusDto } from './bindings/SleepStatusDto';

class SleepStore {
  status = $state<SleepStatusDto | null>(null);
  #backend: Backend | null = null;

  async init(backend: Backend) {
    this.#backend = backend;
    await backend.onSleep((s) => (this.status = s.status));
    this.status = (await backend.sleepGet()).status;
  }

  set = (setting: SleepSettingDto) => this.#backend?.sleepSet(setting);
  cancel = () => this.#backend?.sleepCancel();
  extend = (minutes: number) => this.#backend?.sleepExtend(minutes);
}

export const sleep = new SleepStore();
