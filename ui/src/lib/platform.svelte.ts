import type { Backend } from './backend';

/** The system the app runs on. Starts as Windows (what the app was made for) until the backend says otherwise. */
export const platform = $state<{ os: 'windows' | 'linux' | 'other' }>({ os: 'windows' });

export async function initPlatform(backend: Backend): Promise<void> {
  try {
    const os = await backend.platform();
    platform.os = os === 'windows' || os === 'linux' ? os : 'other';
  } catch {
    platform.os = 'windows';
  }
}
