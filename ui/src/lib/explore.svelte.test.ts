import { describe, expect, it } from 'vitest';
import type { Backend } from './backend';
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
  it("YouTube Music's own shelves load first, under its titles, and refresh asks again", async () => {
    const m = backendWith([], []);
    const s = new ExploreStore();
    expect(s.browseStatus).toBe('idle');
    await s.ensure(m as Backend);
    expect(s.browseStatus).toBe('ready');
    expect(s.browse.map((x) => [x.kind, x.title])).toEqual([
      ['songs', 'Trending'],
      ['albums', 'New albums & singles'],
    ]);
    await s.ensure(m as Backend);
    expect(m.exploreCalls).toBe(1);
    await s.refresh();
    expect(m.exploreCalls).toBe(2);
  });

  it('a failed Explore page says why and still lets the personal shelves load', async () => {
    const m = backendWith([DEMO_TRACKS[0]!], []);
    m.exploreFailure = { kind: 'other', message: 'YouTube Music explore failed (HTTP 400)' };
    const s = new ExploreStore();
    await s.load(m as Backend);
    expect(s.browseStatus).toBe('error');
    expect(s.browseError).toBe('YouTube Music explore failed (HTTP 400)');
    expect(s.browse).toEqual([]);
    expect(s.shelves.map((x) => x.status)).toEqual(['ready']);
  });

  it('offline or rate limited on the Explore page skips the personal shelves instead of asking again', async () => {
    const m = backendWith([DEMO_TRACKS[0]!], [DEMO_TRACKS[1]!]);
    m.exploreFailure = { kind: 'rateLimited', message: 'The music service is limiting requests right now.' };
    const s = new ExploreStore();
    await s.load(m as Backend);
    expect(s.browseStatus).toBe('error');
    expect(s.shelves.map((x) => x.status)).toEqual(['error', 'error']);
    expect(s.shelves[0]!.error).toBe('The music service is limiting requests right now.');
    expect(m.relatedCalls).toHaveLength(0);
  });
});
