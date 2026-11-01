import type { Backend } from './backend';
import { parseSeekMode, type SeekMode } from './seekbar';

/** Which way the seek bar fills in (`PYXEE_SEEK_MODE`, Settings > Advanced). Read once at start. */
export const seekMode = $state<{ value: SeekMode }>({ value: 'step' });

export async function initSeekMode(backend: Backend): Promise<void> {
  try {
    seekMode.value = parseSeekMode(await backend.seekMode());
  } catch {
    seekMode.value = 'step';
  }
}
