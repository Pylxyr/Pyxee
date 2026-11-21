/** What the page asks of its window: full screen (F11), and to hear when the window changed size. The frame itself is the system's. */
export interface WindowControls {
  isFullscreen(): Promise<boolean>;
  setFullscreen(on: boolean): Promise<void>;
  /** Calls `cb` whenever the window changed size (which includes being maximized, restored or put in full screen); resolves to an unsubscribe function. */
  onResized(cb: () => void): Promise<() => void>;
}

/** Only the real app has a window of its own; in a browser (dev server, tests) there is no frame to draw. */
export function hasOwnWindow(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function tauriWindowControls(): Promise<WindowControls> {
  const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
  const here = getCurrentWebviewWindow();
  return {
    isFullscreen: () => here.isFullscreen(),
    setFullscreen: (on) => here.setFullscreen(on),
    onResized: (cb) => here.onResized(() => cb()),
  };
}
