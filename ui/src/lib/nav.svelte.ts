import type { TrackDto } from './bindings/TrackDto';
import { withViewTransition } from './motion';

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

/** Pages in the order they sit in: a page after the current one slides in from the right, before it from the left. */
const ORDER: Route[] = ['home', 'explore', 'library', 'playlist', 'queue', 'search'];

class NavStore {
  route = $state<Route>('home');
  query = $state('');
  nowPlayingOpen = $state(false);
  tab = $state<NowPlayingTab>('upnext');
  /** Track whose row menu is open (in-page sheet, never a native menu). */
  menuTrack = $state<TrackDto | null>(null);
  themeSheetOpen = $state(false);
  sleepSheetOpen = $state(false);
  /** The theme editor (opened from the Settings sheet). */
  themeEditorOpen = $state(false);
  /** Song being added to a playlist (opens the playlist picker). */
  pickTrack = $state<TrackDto | null>(null);
  namePrompt = $state<NamePrompt | null>(null);
  libraryTab = $state<LibraryTab>('liked');
  playlistId = $state<number | null>(null);

  /** True while a view transition carries the cover into (or out of) Now Playing; its own slide then stands down. */
  morphing = $state(false);

  /** Shows a page, sliding it past the old one along the axis the pages are ordered on. */
  #show(route: Route, before?: () => void) {
    if (route === this.route) {
      before?.();
      return;
    }
    const dir = ORDER.indexOf(route) >= ORDER.indexOf(this.route) ? 1 : -1;
    const host = () => document.querySelector<HTMLElement>('.page-host');
    withViewTransition(
      () => {
        before?.();
        this.route = route;
      },
      {
        dir,
        before: () => host()?.style.setProperty('view-transition-name', 'page'),
        after: () => host()?.style.removeProperty('view-transition-name'),
      },
    );
  }
  go(route: Route) {
    if (route !== this.route) this.#show(route);
  }
  search(query: string) {
    this.query = query.trim();
    if (this.query) this.#show('search');
  }
  openPlaylist(id: number) {
    this.#show('playlist', () => (this.playlistId = id));
  }
  openThemeEditor() {
    this.themeEditorOpen = true;
  }
  closeThemeEditor() {
    this.themeEditorOpen = false;
  }
  /** The cover on the deck or dock grows into the big one: a container transform, where the browser can. */
  openNowPlaying() {
    if (this.nowPlayingOpen) return;
    const source = () => document.querySelector<HTMLElement>('[data-np-source]');
    withViewTransition(
      () => (this.nowPlayingOpen = true),
      {
        before: () => {
          this.morphing = true;
          source()?.style.setProperty('view-transition-name', 'np-cover');
        },
        during: () => source()?.style.removeProperty('view-transition-name'),
        after: () => (this.morphing = false),
      },
    );
  }
  /** And back: the big cover shrinks into the deck's. Given a page, it closes onto that page. */
  closeNowPlaying(then?: Route) {
    if (!this.nowPlayingOpen) return;
    const source = () => document.querySelector<HTMLElement>('[data-np-source]');
    withViewTransition(
      () => {
        this.nowPlayingOpen = false;
        if (then) this.route = then;
      },
      {
        before: () => (this.morphing = true),
        during: () => source()?.style.setProperty('view-transition-name', 'np-cover'),
        after: () => {
          this.morphing = false;
          source()?.style.removeProperty('view-transition-name');
        },
      },
    );
  }
}

export const nav = new NavStore();
