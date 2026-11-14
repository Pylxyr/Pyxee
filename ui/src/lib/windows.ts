/** Which window this page is running in: Tauri opens the flyout and the mini player with `?view=...`. */
export type View = 'main' | 'flyout' | 'mini';

export function viewFromSearch(search: string): View {
  const v = new URLSearchParams(search).get('view');
  return v === 'flyout' || v === 'mini' ? v : 'main';
}
