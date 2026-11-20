import { hasOwnWindow } from './windowControls';

/** How big the interface is drawn: a fixed percentage, or `auto` (leave ordinary screens alone, enlarge very large ones). */
export type UiScale = 'auto' | '100' | '125' | '150' | '200';

export const UI_SCALES: { id: UiScale; label: string }[] = [
  { id: 'auto', label: 'Automatic' },
  { id: '100', label: '100%' },
  { id: '125', label: '125%' },
  { id: '150', label: '150%' },
  { id: '200', label: '200%' },
];

export const isUiScale = (v: unknown): v is UiScale => UI_SCALES.some((s) => s.id === v);

/**
 * The page zoom for a choice. `cssWidth` is the window's width at zoom 1 and `scaledByWindows` says whether Windows
 * already enlarges the screen (a display scale above 100% means the person has chosen a size). Automatic only acts on a
 * wide window at 100% display scale (a 4K monitor left unscaled): there it grows in 25% steps to about 1920 px of
 * room, up to 200%. Everywhere else it changes nothing.
 */
export function zoomFor(choice: UiScale, cssWidth: number, scaledByWindows: boolean): number {
  if (choice !== 'auto') return Number(choice) / 100;
  if (scaledByWindows || cssWidth < 2400) return 1;
  return Math.min(2, Math.max(1, Math.floor((cssWidth / 1920) * 4) / 4));
}

// The zoom the window is at now. Page zoom also changes `innerWidth` and `devicePixelRatio`, so both are taken back to
// what they are at zoom 1 before deciding (otherwise automatic would chase its own result).
let current = 1;

/** Applies the choice to the window. Only the real app can (a browser has its own zoom); a failure leaves the size as it is. */
export async function applyUiScale(choice: UiScale): Promise<void> {
  if (!hasOwnWindow()) return;
  const target = zoomFor(choice, window.innerWidth * current, window.devicePixelRatio / current > 1.1);
  if (target === current) return;
  try {
    const { getCurrentWebview } = await import('@tauri-apps/api/webview');
    await getCurrentWebview().setZoom(target);
    current = target;
  } catch {
    /* the size stays as it was */
  }
}

/** Applies the choice now and again when the window is resized (automatic depends on the window's width). Returns a stop function. */
export function watchUiScale(getChoice: () => UiScale): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const onResize = () => {
    clearTimeout(timer);
    timer = setTimeout(() => void applyUiScale(getChoice()), 250);
  };
  window.addEventListener('resize', onResize);
  void applyUiScale(getChoice());
  return () => {
    clearTimeout(timer);
    window.removeEventListener('resize', onResize);
  };
}
