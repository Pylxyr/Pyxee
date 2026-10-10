import { messageOf, type Backend } from './backend';
import type { LibrarySnapshotDto } from './bindings/LibrarySnapshotDto';
import type { PlaylistSummaryDto } from './bindings/PlaylistSummaryDto';
import type { TrackDto } from './bindings/TrackDto';
import { snackbar } from './snackbar.svelte';

const EMPTY: LibrarySnapshotDto = { revision: 0, likedKeys: [], playlists: [], historyCount: 0, error: null };

/** What the UI needs to know about the library at a glance. Detail views fetch their own data. */
export class LibraryStore {
  snapshot = $state<LibrarySnapshotDto>(EMPTY);
  ready = $state(false);
  #backend: Backend | null = null;
  #unlisten: (() => void) | null = null;

  likedSet = $derived(new Set(this.snapshot.likedKeys));
  playlists = $derived<PlaylistSummaryDto[]>(this.snapshot.playlists);
  revision = $derived(this.snapshot.revision);

  async init(backend: Backend) {
    this.#unlisten?.();
    this.#backend = backend;
    this.#unlisten = await backend.onLibrary((s) => (this.snapshot = s));
    this.snapshot = await backend.getLibrary();
    this.ready = true;
  }

  /** For detail views that load their own data (liked songs, a playlist, history). */
  get backend(): Backend | null {
    return this.#backend;
  }

  isLiked(key: string): boolean {
    return this.likedSet.has(key);
  }

  /** Likes or unlikes a song; failures are shown, never swallowed. */
  async toggleLike(track: TrackDto) {
    const liked = !this.isLiked(track.key);
    try {
      await this.#backend?.setLiked(track, liked);
    } catch (e) {
      snackbar.push(messageOf(e));
    }
  }

  /** Adds songs to a playlist and says what happened. */
  async addToPlaylist(playlist: { id: number; name: string }, tracks: TrackDto[]) {
    try {
      const r = await this.#backend!.playlistAdd(playlist.id, tracks);
      snackbar.push(
        r.added === 0 ? `Already in ${playlist.name}` : r.skipped > 0 ? `Added ${r.added} to ${playlist.name} (${r.skipped} already there)` : `Added to ${playlist.name}`,
      );
    } catch (e) {
      snackbar.push(messageOf(e));
    }
  }
}

export const library = new LibraryStore();
