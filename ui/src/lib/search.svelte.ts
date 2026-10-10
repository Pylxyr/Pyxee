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

/** Search state. Late answers for an older query are dropped, so results never flicker back. */
export class SearchStore {
  query = $state('');
  filter = $state<SearchFilterDto>('all');
  status = $state<SearchStatus>('idle');
  items = $state<SearchItemDto[]>([]);
  error = $state<SearchErrorDto | null>(null);
  filters = $state<FilterInfoDto[]>(DEFAULT_FILTERS);
  suggestions = $state<string[]>([]);
  #backend: Backend | null = null;
  #seq = 0;
  #suggestSeq = 0;
  #timer: ReturnType<typeof setTimeout> | null = null;

  init(backend: Backend) {
    this.#backend = backend;
    backend.searchFilters().then((f) => (this.filters = f)).catch(() => {});
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

export const search = new SearchStore();
