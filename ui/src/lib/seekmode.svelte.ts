import { persist } from './persist';
import type { Backend } from './backend';
import { parseSeekMode, SEEK_MODES, type SeekMode } from './seekbar';

const KEY = 'pyxee.seek';

/** The way the bar moves unless the person (or a developer build's `PYXEE_SEEK_MODE`) picked another: one compositor animation per song. */
export const DEFAULT_SEEK_MODE: SeekMode = 'glide';

function saved(): SeekMode | null {
  try {
    const v = localStorage.getItem(KEY);
    return (SEEK_MODES as string[]).includes(v ?? '') ? (v as SeekMode) : null;
  } catch {
    return null;
  }
}

/** How the seek bar fills in (Settings > Appearance). The saved choice wins; then a developer's `PYXEE_SEEK_MODE`; then the default. */
export const seekMode = $state<{ value: SeekMode }>({ value: saved() ?? DEFAULT_SEEK_MODE });

export function setSeekMode(v: SeekMode): void {
  seekMode.value = v;
  try {
    localStorage.setItem(KEY, v);
    persist.touch();
  } catch {
    /* ignore */
  }
}

export async function initSeekMode(backend: Backend): Promise<void> {
  if (saved()) return;
  try {
    const env = parseSeekMode(await backend.seekMode());
    // `step` is what an unset variable (and every release build) answers, so it says nothing; the others were asked for.
    if (env !== 'step') seekMode.value = env;
  } catch {
    /* keep the default */
  }
}
