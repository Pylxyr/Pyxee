import type { TrackDto } from './bindings/TrackDto';

export type Route = 'home' | 'explore' | 'library' | 'playlist' | 'queue' | 'search';
export type LibraryTab = 'liked' | 'playlists' | 'history' | 'downloads';

/** An in-page prompt for a name (new playlist, rename, save queue). Never a native dialog. */
export interface NamePrompt {
  title: string;
  initial: string;
  confirm: string;
  /** Rejects with a sentence to show under the field; resolving closes the prompt. */
  run: (name: string) => Promise<void>;
}
export type NowPlayingTab = 'upnext' | 'lyrics' | 'related';

class NavStore {
  route = $state<Route>('home');
  query = $state('');
  nowPlayingOpen = $state(false);
  tab = $state<NowPlayingTab>('upnext');
  /** Track whose row menu is open (in-page sheet, never a native menu). */
  menuTrack = $state<TrackDto | null>(null);
  themeSheetOpen = $state(false);
  /** The theme editor (opened from the Settings sheet). */
  themeEditorOpen = $state(false);
  /** Song being added to a playlist (opens the playlist picker). */
  pickTrack = $state<TrackDto | null>(null);
  namePrompt = $state<NamePrompt | null>(null);
  libraryTab = $state<LibraryTab>('liked');
  playlistId = $state<number | null>(null);

  go(route: Route) {
    this.route = route;
  }
  search(query: string) {
    this.query = query.trim();
    if (this.query) this.route = 'search';
  }
  openPlaylist(id: number) {
    this.playlistId = id;
    this.route = 'playlist';
  }
  openThemeEditor() {
    this.themeEditorOpen = true;
  }
  closeThemeEditor() {
    this.themeEditorOpen = false;
  }
  openNowPlaying() {
    this.nowPlayingOpen = true;
  }
  closeNowPlaying() {
    this.nowPlayingOpen = false;
  }
}

export const nav = new NavStore();
