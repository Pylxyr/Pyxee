import type { Backend } from './backend';
import { toSearchError } from './backend';
import type { FilterInfoDto } from './bindings/FilterInfoDto';
import type { SearchErrorDto } from './bindings/SearchErrorDto';
import type { SearchFilterDto } from './bindings/SearchFilterDto';
import type { SearchItemDto } from './bindings/SearchItemDto';

export type SearchStatus = 'idle' | 'loading' | 'ready' | 'error';

const DEFAULT_FILTERS: FilterInfoDto[] = (['all', 'songs', 'videos', 'albums', 'artists', 'playlists'] as const).map(
  (filter) => ({ filter, supported: true }),
);
export const SUGGEST_DELAY_MS = 150;
export const FILTER_KEY = 'pyxee.searchFilter';
/** Songs unless the user picked something else last time. */
export const DEFAULT_FILTER: SearchFilterDto = 'songs';

type Store = Pick<Storage, 'getItem' | 'setItem'>;

function loadFilter(storage: Store | null): SearchFilterDto {
  try {
    const v = storage?.getItem(FILTER_KEY);
    if (v && DEFAULT_FILTERS.some((f) => f.filter === v)) return v as SearchFilterDto;
  } catch {
    /* storage unavailable: use the default */
  }
  return DEFAULT_FILTER;
}

/** Search state. Late answers for an older query are dropped, so results never flicker back. */
export class SearchStore {
  query = $state('');
  filter = $state<SearchFilterDto>(DEFAULT_FILTER);
  status = $state<SearchStatus>('idle');
  items = $state<SearchItemDto[]>([]);
  error = $state<SearchErrorDto | null>(null);
  filters = $state<FilterInfoDto[]>(DEFAULT_FILTERS);
  suggestions = $state<string[]>([]);
  #backend: Backend | null = null;
  #seq = 0;
  #suggestSeq = 0;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #storage: Store | null;

  constructor(storage: Store | null = null) {
    this.#storage = storage;
    this.filter = loadFilter(storage);
  }

  init(backend: Backend) {
    this.#backend = backend;
    backend
      .searchFilters()
      .then((f) => {
        this.filters = f;
        // A remembered filter this source cannot do any more falls back instead of finding nothing.
        const ok = (x: SearchFilterDto) => f.find((i) => i.filter === x)?.supported;
        if (!ok(this.filter)) this.filter = ok(DEFAULT_FILTER) ? DEFAULT_FILTER : 'all';
      })
      .catch(() => {});
  }

  async run(query: string, filter: SearchFilterDto = this.filter) {
    const q = query.trim();
    this.query = q;
    this.filter = filter;
    this.clearSuggestions();
    const seq = ++this.#seq;
    if (!q || !this.#backend) {
      this.status = 'idle';
      this.items = [];
      this.error = null;
      return;
    }
    this.status = 'loading';
    this.error = null;
    try {
      const r = await this.#backend.search(q, filter);
      if (seq !== this.#seq) return;
      this.items = r.items;
      this.status = 'ready';
    } catch (e) {
      if (seq !== this.#seq) return;
      this.items = [];
      this.error = toSearchError(e);
      this.status = 'error';
    }
  }

  retry() {
    return this.run(this.query, this.filter);
  }

  setFilter(filter: SearchFilterDto) {
    if (!this.filters.find((f) => f.filter === filter)?.supported) return Promise.resolve();
    // Only a choice made on the chips is remembered; "Go to artist" and the like just search.
    try {
      this.#storage?.setItem(FILTER_KEY, filter);
    } catch {
      /* not remembered, still works */
    }
    return this.run(this.query, filter);
  }

  /** Called on every keystroke; asks for suggestions after a short pause. */
  typing(prefix: string) {
    if (this.#timer) clearTimeout(this.#timer);
    const seq = ++this.#suggestSeq;
    if (!prefix.trim() || !this.#backend) {
      this.suggestions = [];
      return;
    }
    this.#timer = setTimeout(async () => {
      const list = await this.#backend!.suggest(prefix).catch(() => []);
      if (seq === this.#suggestSeq) this.suggestions = list;
    }, SUGGEST_DELAY_MS);
  }

  clearSuggestions() {
    if (this.#timer) clearTimeout(this.#timer);
    this.#suggestSeq++;
    this.suggestions = [];
  }
}

export const search = new SearchStore(typeof localStorage !== 'undefined' ? localStorage : null);
