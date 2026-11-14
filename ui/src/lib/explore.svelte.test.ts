import { describe, expect, it } from 'vitest';
import type { Backend } from './backend';
import type { ExploreShelfDto } from './bindings/ExploreShelfDto';
import type { TrackDto } from './bindings/TrackDto';
import { ExploreStore } from './explore.svelte';
import { DEMO_TRACKS, MockBackend } from './mock';

const hist = (tracks: TrackDto[]) => tracks.map((track, i) => ({ track, playedAt: 100 - i }));

function backendWith(liked: TrackDto[], history: TrackDto[]) {
  const m = new MockBackend();
  m.likedGet = async () => liked;
  m.historyGet = async () => hist(history);
  return m;
}

describe('ExploreStore', () => {
  it('builds shelves from likes and plays and does not repeat songs between them', async () => {
    const m = backendWith([DEMO_TRACKS[0]!], [DEMO_TRACKS[1]!, DEMO_TRACKS[2]!]);
    const s = new ExploreStore();
    await s.load(m as Backend);
    expect(s.status).toBe('ready');
    expect(s.shelves.map((x) => x.title)).toEqual([
      `Because you liked ${DEMO_TRACKS[0]!.title}`,
      `Because you played ${DEMO_TRACKS[1]!.title}`,
      `Because you played ${DEMO_TRACKS[2]!.title}`,
    ]);
    const keys = s.shelves.flatMap((x) => x.tracks.map((t) => t.key));
    expect(new Set(keys).size).toBe(keys.length);
    expect(keys).not.toContain(DEMO_TRACKS[0]!.key);
    expect(s.jump.length).toBe(2);
  });

  it('with no likes or plays it has no shelves and says so', async () => {
    const s = new ExploreStore();
    await s.load(backendWith([], []) as Backend);
    expect(s.noSeeds).toBe(true);
    expect(s.shelves).toEqual([]);
  });

  it('an offline failure marks the shelf and skips the rest instead of asking again', async () => {
    const m = backendWith([DEMO_TRACKS[0]!], [DEMO_TRACKS[1]!]);
    m.relatedFailure = { kind: 'offline', message: "You're offline." };
    const s = new ExploreStore();
    await s.load(m as Backend);
    expect(s.shelves.map((x) => x.status)).toEqual(['error', 'error']);
    expect(s.shelves[1]!.error).toBe("You're offline.");
    expect(m.relatedCalls).toHaveLength(1);
  });

  it('ensure loads once; refresh loads again', async () => {
    const m = backendWith([DEMO_TRACKS[0]!], []);
    const s = new ExploreStore();
    await s.ensure(m as Backend);
    await s.ensure(m as Backend);
    expect(m.relatedCalls).toHaveLength(1);
    await s.refresh();
    expect(m.relatedCalls).toHaveLength(2);
  });

  it('an answer for an older load is dropped', async () => {
    const m = backendWith([DEMO_TRACKS[0]!], []);
    const s = new ExploreStore();
    await Promise.all([s.load(m as Backend), s.load(m as Backend)]);
    expect(s.shelves).toHaveLength(1);
    expect(s.shelves[0]!.status).toBe('ready');
  });
  it('loads the genres once, favourites first, and a failure only hides them', async () => {
    const m = backendWith([], []);
    const s = new ExploreStore();
    await s.ensure(m as Backend);
    expect(s.genresStatus).toBe('ready');
    expect(s.genres.map((g) => g.title)).toEqual(['Anime', 'J-Pop', 'K-Pop', 'Chill', 'Rock']);
    await s.refresh();
    expect(m.genresCalls).toBe(1);

    const down = backendWith([], []);
    down.genreFailure = { kind: 'offline', message: 'offline' };
    const t = new ExploreStore();
    await t.load(down as Backend);
    expect(t.genresStatus).toBe('error');
    expect(t.genres).toEqual([]);
  });

  it('a genre shows its own shelves, asks once per session, and null goes back to the usual page', async () => {
    const m = backendWith([], []);
    const s = new ExploreStore();
    await s.load(m as Backend);
    const kpop = s.genres.find((g) => g.title === 'K-Pop')!;
    await s.selectGenre(m as Backend, kpop);
    expect(s.selectedGenre).toEqual(kpop);
    expect(s.genreStatus).toBe('ready');
    expect(s.genreShelves.map((x) => x.kind)).toEqual(['playlists', 'songs']);
    await s.selectGenre(m as Backend, null);
    expect(s.genreShelves).toEqual([]);
    await s.selectGenre(m as Backend, kpop);
    expect(m.genreCalls).toEqual([kpop.params]);
    await s.refresh();
    await s.selectGenre(m as Backend, kpop);
    expect(m.genreCalls).toEqual([kpop.params, kpop.params]);
  });

  it('a failed genre says why and can be tried again', async () => {
    const m = backendWith([], []);
    const s = new ExploreStore();
    await s.load(m as Backend);
    const anime = s.genres[0]!;
    m.genreFailure = { kind: 'rateLimited', message: 'The music service is limiting requests right now.' };
    await s.selectGenre(m as Backend, anime);
    expect(s.genreStatus).toBe('error');
    expect(s.genreError).toBe('The music service is limiting requests right now.');
    m.genreFailure = null;
    await s.retryGenre(m as Backend);
    expect(s.genreStatus).toBe('ready');
    expect(s.genreError).toBeNull();
  });

  it('an answer for a genre that is no longer picked is dropped', async () => {
    const m = backendWith([], []);
    const s = new ExploreStore();
    await s.load(m as Backend);
    const [first, second] = [s.genres[0]!, s.genres[1]!];
    let release: (v: ExploreShelfDto[]) => void = () => {};
    const slow = new Promise<ExploreShelfDto[]>((r) => (release = r));
    const real = m.genreGet.bind(m);
    m.genreGet = (params: string) => (params === first.params ? slow : real(params));
    const pending = s.selectGenre(m as Backend, first);
    await s.selectGenre(m as Backend, second);
    release([{ kind: 'songs', id: 'late', title: 'Late', tracks: [] }]);
    await pending;
    expect(s.selectedGenre).toEqual(second);
    expect(s.genreShelves.map((x) => x.id)).not.toContain('late');
  });
});
