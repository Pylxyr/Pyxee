import type { Backend } from './backend';

export type Os = 'windows' | 'linux' | 'other';

/** The system named by a browser user-agent string. WebView2 says Windows, WebKitGTK says Linux; Android is not Linux here. */
export function guessOs(userAgent: string): Os {
  if (/windows/i.test(userAgent)) return 'windows';
  if (/android/i.test(userAgent)) return 'other';
  if (/linux|x11|bsd/i.test(userAgent)) return 'linux';
  return 'other';
}

function initialOs(): Os {
  return typeof navigator === 'undefined' ? 'windows' : guessOs(navigator.userAgent);
}

/**
 * The system the app runs on. Starts as the web view's own guess (so a Linux window never flashes Windows wording
 * before the backend has answered) and is replaced by what the backend says.
 */
export const platform = $state<{ os: Os }>({ os: initialOs() });

export async function initPlatform(backend: Backend): Promise<void> {
  try {
    const os = await backend.platform();
    platform.os = os === 'windows' || os === 'linux' ? os : 'other';
  } catch {
    // Keep the guess.
  }
}
