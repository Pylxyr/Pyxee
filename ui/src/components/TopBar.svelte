<script lang="ts">
  import { nav } from '$lib/nav.svelte';
  import { search } from '$lib/search.svelte';
  import Icon from './Icon.svelte';

  let text = $state('');
  let active = $state(-1);
  const open = $derived(search.suggestions.length > 0);

  function go(q: string) {
    text = q;
    active = -1;
    nav.go('search');
    search.run(q);
  }
  function submit(e: SubmitEvent) {
    e.preventDefault();
    go(active >= 0 ? (search.suggestions[active] ?? text) : text);
  }
  function oninput() {
    active = -1;
    search.typing(text);
  }
  function onkeydown(e: KeyboardEvent) {
    const n = search.suggestions.length;
    if (e.key === 'ArrowDown' && n) {
      e.preventDefault();
      active = (active + 1) % n;
    } else if (e.key === 'ArrowUp' && n) {
      e.preventDefault();
      active = (active - 1 + n) % n;
    } else if (e.key === 'Escape' && n) {
      e.stopPropagation();
      search.clearSuggestions();
      active = -1;
    }
  }
</script>

<header class="px-4 py-3 sm:px-8">
  <form class="relative max-w-xl" role="search" onsubmit={submit}>
    <div class="flex h-10 items-center gap-3 rounded-md border border-line bg-surface px-3 focus-within:border-accent">
      <Icon name="search" size={20} />
      <input
        type="search"
        role="combobox"
        aria-expanded={open}
        aria-controls="search-suggestions"
        aria-autocomplete="list"
        aria-activedescendant={active >= 0 ? `suggest-${active}` : undefined}
        autocomplete="off"
        bind:value={text}
        {oninput}
        {onkeydown}
        onblur={() => setTimeout(() => search.clearSuggestions(), 120)}
        class="min-w-0 flex-1 bg-transparent text-[15px] outline-none placeholder:text-muted"
        placeholder="Search songs, albums, artists"
        aria-label="Search"
      />
    </div>
    {#if open}
      <ul id="search-suggestions" role="listbox" aria-label="Suggestions" class="absolute inset-x-0 top-12 z-20 overflow-hidden rounded-md border border-line bg-solid-2 py-1 shadow-xl backdrop-blur">
        {#each search.suggestions as s, i (s)}
          <li
            id="suggest-{i}"
            role="option"
            aria-selected={active === i}
            class="flex cursor-pointer items-center gap-3 px-4 py-2.5 text-[15px] {active === i ? 'bg-surface' : ''}"
            onmousedown={(e) => {
              e.preventDefault();
              go(s);
            }}
          >
            <Icon name="search" size={16} />{s}
          </li>
        {/each}
      </ul>
    {/if}
  </form>
</header>
