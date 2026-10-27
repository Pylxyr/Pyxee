<script lang="ts">
  import { nav } from '$lib/nav.svelte';
  import { search } from '$lib/search.svelte';
  import Icon from './Icon.svelte';
  import IconButton from './IconButton.svelte';

  // The search box is the same place at every window size: the top of the page. Narrow windows, which have no tabs,
  // also keep the Settings button here.
  let { showSettings = false }: { showSettings?: boolean } = $props();

  let text = $state('');
  let active = $state(-1);
  const open = $derived(search.suggestions.length > 0);

  // The suggestions go a moment after the box loses focus. Coming back to the box inside that moment keeps them:
  // the late clear must not wipe what was typed since.
  let blurTimer: ReturnType<typeof setTimeout> | undefined;

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

<header class="topbar page-pad">
  <form class="relative min-w-0 max-w-xl flex-1 2xl:max-w-3xl" role="search" onsubmit={submit}>
    <div class="search-box">
      <Icon name="search" size={18} />
      <input
        type="search"
        role="combobox"
        aria-expanded={open}
        aria-controls="search-suggestions"
        aria-autocomplete="list"
        aria-activedescendant={active >= 0 ? `suggest-${active}` : undefined}
        autocomplete="off"
        spellcheck="false"
        bind:value={text}
        {oninput}
        {onkeydown}
        onfocus={() => clearTimeout(blurTimer)}
        onblur={() => (blurTimer = setTimeout(() => search.clearSuggestions(), 120))}
        class="search-input min-w-0 flex-1 bg-transparent text-fg outline-none placeholder:text-muted"
        placeholder="Search songs, albums, artists"
        aria-label="Search"
      />
    </div>
    {#if open}
      <ul id="search-suggestions" role="listbox" aria-label="Suggestions" class="popover absolute inset-x-0 top-11 z-20 overflow-hidden bg-solid-2 py-1 backdrop-blur">
        {#each search.suggestions as s, i (s)}
          <li
            id="suggest-{i}"
            role="option"
            aria-selected={active === i}
            class="suggestion"
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
  {#if showSettings}
    <IconButton icon="settings" label="Settings" size="lg" onclick={() => (nav.themeSheetOpen = true)} />
  {/if}
</header>
