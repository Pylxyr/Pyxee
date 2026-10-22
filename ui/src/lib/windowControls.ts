/** The buttons of the window frame. The main window has no native title bar, so the page draws them and calls these. */
export interface WindowControls {
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  /** Asks the window to close, like the native X did: with close-to-tray on, it only hides. */
  close(): Promise<void>;
  isMaximized(): Promise<boolean>;
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
    minimize: () => here.minimize(),
    toggleMaximize: () => here.toggleMaximize(),
    close: () => here.close(),
    isMaximized: () => here.isMaximized(),
    isFullscreen: () => here.isFullscreen(),
    setFullscreen: (on) => here.setFullscreen(on),
    onResized: (cb) => here.onResized(() => cb()),
  };
}
