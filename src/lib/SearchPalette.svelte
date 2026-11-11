<script lang="ts">
  import Callout from "./Callout.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import { searchAniList } from "./api";
  import { coverOf, displayTitle, metaOf, scoreOutOfTen } from "./format";
  import { incognitoEnabled } from "./incognito";
  import {
    forgetRecentSearches,
    loadRecentSearches,
    rememberSearch,
  } from "./recentSearches";
  import type { LibraryItem, Media, SourceLibraryItem, View } from "./types";

  let {
    open,
    incognito = false,
    library,
    sourceLibrary = [],
    onClose,
    onOpen,
    onPlay,
    onOpenSource,
    onNav,
    onOpenShortcuts,
  }: {
    open: boolean;
    incognito?: boolean;
    library: LibraryItem[];
    sourceLibrary?: SourceLibraryItem[];
    onClose: () => void;
    onOpen: (item: Media) => void;
    onPlay: (item: Media) => void;
    onOpenSource: (item: SourceLibraryItem) => void;
    onNav: (view: View) => void;
    onOpenShortcuts: () => void;
  } = $props();

  const ACTIONS = [
    { id: "browse", label: "Browse trending", hint: "1", icon: "grid", view: "browse" as View },
    { id: "library", label: "Go to Library", hint: "2", icon: "library", view: "library" as View },
    { id: "history", label: "Open History", hint: "3", icon: "clock", view: "history" as View },
    { id: "sources", label: "Manage Sources", hint: "4", icon: "package", view: "sources" as View },
    { id: "settings", label: "Open Settings", hint: "5", icon: "settings", view: "settings" as View },
    { id: "shortcuts", label: "Keyboard shortcuts", hint: "?", icon: "keyboard", view: null },
  ];

  let query = $state("");
  let results = $state<Media[]>([]);
  let lastGood = $state<Media[]>([]);
  let searching = $state(false);
  let error = $state<string | null>(null);
  let rateLimited = $state(false);
  let requestId = $state(0);
  const initialRecent = loadRecentSearches();
  let recent = $state<string[]>(initialRecent);
  // The last list that was written to storage. Incognito entries live only in
  // `recent` and are dropped when the mode is turned off, so a later normal
  // search can never persist them.
  let persistedRecent = initialRecent;
  let hasPrivateRecent = $state(false);
  let active = $state(0);
  let dialog: HTMLDivElement | null = $state(null);

  type Row =
    | { kind: "lib"; item: Media }
    | { kind: "net"; item: Media }
    | { kind: "source"; item: SourceLibraryItem }
    | { kind: "action"; id: string; label: string; hint: string; icon: string; view: View | null };

  const term = $derived(query.trim());

  const libHits = $derived(
    term.length >= 2
      ? library
          .filter((entry) => displayTitle(entry.media).toLowerCase().includes(term.toLowerCase()))
          .slice(0, 3)
          .map((entry) => entry.media)
      : [],
  );

  const sourceHits = $derived(
    term.length >= 2
      ? sourceLibrary
          .filter((entry) => entry.title.toLowerCase().includes(term.toLowerCase()))
          .slice(0, 3)
      : [],
  );

  const actionHits = $derived(
    term.length >= 2
      ? ACTIONS.filter((action) => action.label.toLowerCase().includes(term.toLowerCase()))
      : ACTIONS,
  );

  const groups = $derived.by(() => {
    const out: { label: string; rows: Row[] }[] = [];
    if (term.length >= 2) {
      if (libHits.length > 0) {
        out.push({ label: "Library", rows: libHits.map((item): Row => ({ kind: "lib", item })) });
      }
      if (results.length > 0) {
        out.push({ label: "AniList", rows: results.map((item): Row => ({ kind: "net", item })) });
      }
      if (sourceHits.length > 0) {
        out.push({ label: "Sources", rows: sourceHits.map((item): Row => ({ kind: "source", item })) });
      }
      if (actionHits.length > 0) {
        out.push({
          label: "Actions",
          rows: actionHits.map((action): Row => ({ kind: "action", ...action })),
        });
      }
    } else {
      out.push({ label: "Jump to", rows: ACTIONS.map((action): Row => ({ kind: "action", ...action })) });
    }
    return out;
  });

  const flatRows = $derived(groups.flatMap((group) => group.rows));
  const activeDescendant = $derived(
    flatRows.length > 0 ? `palette-row-${active}` : undefined,
  );

  function saveRecent(value: string) {
    if (incognito || incognitoEnabled()) {
      recent = rememberSearch(value, recent, false);
      hasPrivateRecent = true;
      return;
    }
    const next = rememberSearch(value, persistedRecent, true);
    persistedRecent = next;
    recent = next;
  }

  function clearRecent() {
    recent = [];
    persistedRecent = [];
    hasPrivateRecent = false;
    forgetRecentSearches();
  }

  function reset() {
    query = "";
    results = [];
    error = null;
    rateLimited = false;
    active = 0;
  }

  // Reset the query whenever the palette opens. Modal handles focus and restore.
  $effect(() => {
    if (open) {
      reset();
    }
  });

  // Private terms never survive turning Incognito off.
  $effect(() => {
    if (!incognito && hasPrivateRecent) {
      recent = persistedRecent;
      hasPrivateRecent = false;
    }
  });

  $effect(() => {
    term;
    active = 0;
  });

  $effect(() => {
    if (!open || !dialog) return;
    const row = dialog.querySelector(`[data-palette-index="${active}"]`);
    row?.scrollIntoView({ block: "nearest" });
  });

  // Debounced live search. Stale responses are dropped by request id.
  $effect(() => {
    if (!open || term.length < 2) {
      results = [];
      searching = false;
      return;
    }
    searching = true;
    const timer = window.setTimeout(() => {
      const id = ++requestId;
      void (async () => {
        try {
          const found = await searchAniList(term, 1);
          if (id !== requestId) return;
          results = found;
          lastGood = found;
          error = null;
          rateLimited = false;
          active = 0;
        } catch (e) {
          if (id !== requestId) return;
          const message = e instanceof Error ? e.message : String(e);
          error = message;
          rateLimited = /429|rate|retry/i.test(message);
          if (rateLimited && lastGood.length > 0) {
            results = lastGood;
          }
        } finally {
          if (id === requestId) searching = false;
        }
      })();
    }, 200);
    return () => window.clearTimeout(timer);
  });

  function choose(row: Row | undefined) {
    if (!row) return;
    if (row.kind === "action") {
      onClose();
      if (row.view) onNav(row.view);
      else onOpenShortcuts();
      return;
    }
    if (row.kind === "source") {
      onClose();
      onOpenSource(row.item);
      return;
    }
    if (term.length >= 2) saveRecent(term);
    onClose();
    onOpen(row.item);
  }

  function play(row: Row | undefined) {
    if (!row) return;
    if (row.kind === "lib" || row.kind === "net") {
      if (term.length >= 2) saveRecent(term);
      onClose();
      onPlay(row.item);
      return;
    }
    choose(row);
  }

  function onKeys(event: KeyboardEvent) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      active = flatRows.length ? (active + 1) % flatRows.length : 0;
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      active = flatRows.length ? (active - 1 + flatRows.length) % flatRows.length : 0;
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (event.shiftKey) play(flatRows[active]);
      else choose(flatRows[active]);
    }
  }
</script>

<Modal
  {open}
  size="sm"
  placement="top"
  maxHeight={null}
  bodyMaxHeight="56vh"
  zIndex={60}
  overlayName="palette"
  label="Search Kaminari"
  labelledBy="search-palette-title"
  onClose={onClose}
  bind:panel={dialog}
  bodyClass="p-2"
  footerClass="flex flex-wrap gap-4 px-4 py-2.5 text-[10px]"
>
  {#snippet header()}
    <h2 id="search-palette-title" class="sr-only">Search Kaminari</h2>
    <div class="palette-input-row flex items-center gap-3 px-4 h-14">
      <Icon name="search" size={18} class="shrink-0" />
      <input
        bind:value={query}
        onkeydown={onKeys}
        placeholder="Search AniList, library, sources, actions"
        class="palette-input flex-1 h-full bg-transparent text-[15px] outline-none"
        style="color: var(--paper-50);"
        aria-label="Search AniList, library, sources and actions"
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={flatRows.length > 0}
        aria-controls={flatRows.length > 0 ? "palette-results" : undefined}
        aria-activedescendant={activeDescendant}
        autocomplete="off"
        spellcheck={false}
        data-autofocus
      />
      {#if searching}
        <span class="w-3.5 h-3.5 rounded-full border-2 border-t-transparent animate-spin shrink-0" style="border-color: var(--volt-400); border-top-color: transparent;" aria-label="Searching"></span>
      {/if}
      <button class="kbd" onclick={onClose} aria-label="Close search">Esc</button>
    </div>
  {/snippet}

  <p class="sr-only" aria-live="polite">
    {searching
      ? "Searching AniList"
      : term.length < 2
        ? `${flatRows.length} quick actions`
        : `${flatRows.length} results`}
  </p>

  {#if rateLimited}
    <Callout tone="warning" class="m-2">
      AniList is rate limiting. Showing the last good results. Try again in a few seconds.
    </Callout>
  {:else if error}
    <Callout tone="danger" icon="xCircle" class="m-2">
      Search failed: {error}
    </Callout>
  {/if}

  {#if term.length < 2}
    {#if recent.length > 0}
      <div class="flex items-center gap-2 px-3 pt-2 pb-1">
        <p class="text-[10px] font-semibold uppercase tracking-[0.18em]" style="color: var(--paper-500);">Recent</p>
        <span class="flex-1"></span>
        <button class="text-[10px]" style="color: var(--paper-500);" onclick={clearRecent} aria-label="Clear recent searches">Clear</button>
      </div>
      <div class="flex flex-wrap gap-1.5 px-3 pb-2">
        {#each recent as value}
          <button
            class="chip text-[11px] px-2.5 py-1 hover:border-[var(--line-volt)]"
            onclick={() => {
              query = value;
            }}
          >
            <Icon name="clock" size={11} />
            {value}
          </button>
        {/each}
      </div>
      {#if incognito}
        <p class="px-3 pb-2 text-[11px]" style="color: var(--paper-500);">
          Incognito is on. New searches stay in this window and are not saved.
        </p>
      {/if}
    {:else}
      <p class="px-3 py-3 text-sm" style="color: var(--paper-500);">
        Type two or more characters. Library matches show first, AniList next.
      </p>
    {/if}
  {/if}

  {#if flatRows.length === 0 && !searching}
    <div class="px-4 py-8 text-center">
      <div class="empty-icon mx-auto" aria-hidden="true">
        <Icon name="search" size={24} />
      </div>
      <p class="text-sm font-semibold">No results for "{query.trim()}"</p>
      <p class="text-xs mt-1" style="color: var(--paper-500);">Try romaji, a shorter title, or check spelling.</p>
    </div>
  {/if}

  <div
    id="palette-results"
    role="listbox"
    aria-label="Search results"
    aria-hidden={flatRows.length === 0 ? "true" : undefined}
  >
    {#each groups as group (group.label)}
      <div class="mb-1" role="group" aria-label={group.label}>
        <div class="flex items-center gap-2 px-3 pt-2 pb-1" aria-hidden="true">
          <p class="text-[10px] font-semibold uppercase tracking-[0.18em]" style="color: var(--paper-500);">{group.label}</p>
          <span class="text-[10px]" style="color: var(--paper-500); font-family: var(--font-mono);">{group.rows.length}</span>
        </div>
      {#each group.rows as row (row.kind + (row.kind === "action" ? row.id : row.kind === "source" ? row.item.showId : row.item.id))}
        {@const index = flatRows.indexOf(row)}
        {@const isActive = index === active}
        {#if row.kind === "action"}
          <button
            id={`palette-row-${index}`}
            class="palette-row {isActive ? 'is-active' : ''}"
            data-palette-index={index}
            role="option"
            aria-selected={isActive}
            tabindex={-1}
            onmouseenter={() => (active = index)}
            onclick={() => choose(row)}
          >
            <span class="palette-row-icon">
              <Icon name={row.icon} size={15} />
            </span>
            <span class="flex-1 text-sm font-medium">{row.label}</span>
            <kbd class="kbd">{row.hint}</kbd>
          </button>
        {:else if row.kind === "source"}
          <button
            id={`palette-row-${index}`}
            class="palette-row {isActive ? 'is-active' : ''}"
            data-palette-index={index}
            role="option"
            aria-selected={isActive}
            tabindex={-1}
            onmouseenter={() => (active = index)}
            onclick={() => choose(row)}
            aria-label="Open {row.item.title} from {row.item.extensionName}"
          >
            <span class="palette-cover">
              {#if row.item.cover}
                <img src={row.item.cover} alt="" loading="lazy" decoding="async" />
              {:else}
                <Icon name="package" size={16} />
              {/if}
            </span>
            <span class="min-w-0 flex-1">
              <span class="flex items-center gap-2">
                <span class="block text-sm font-semibold truncate">{row.item.title}</span>
                <span class="chip text-[9px] px-1.5 py-0.5">Source</span>
              </span>
              <span class="block text-xs mt-0.5 truncate" style="color: var(--paper-500);">
                {row.item.extensionName}{row.item.year ? ` · ${row.item.year}` : ""}
              </span>
            </span>
            <Icon name="chevronRight" size={14} class="shrink-0" />
          </button>
        {:else}
          {@const art = coverOf(row.item)}
          <button
            id={`palette-row-${index}`}
            class="palette-row {isActive ? 'is-active' : ''}"
            data-palette-index={index}
            role="option"
            aria-selected={isActive}
            tabindex={-1}
            onmouseenter={() => (active = index)}
            onclick={() => choose(row)}
            aria-label="Open {displayTitle(row.item)}"
          >
            <span class="palette-cover" style="background: {row.item.coverImage?.color ?? 'var(--void-700)'};">
              {#if art}
                <img src={art} alt="" loading="lazy" decoding="async" />
              {:else}
                <Icon name="film" size={16} />
              {/if}
            </span>
            <span class="min-w-0 flex-1">
              <span class="flex items-center gap-2">
                <span class="block text-sm font-semibold truncate">{displayTitle(row.item)}</span>
                {#if row.kind === "lib"}
                  <span class="chip chip-volt text-[9px] px-1.5 py-0.5">Library</span>
                {/if}
              </span>
              <span class="block text-xs mt-0.5 truncate" style="color: var(--paper-500);">
                {row.kind === "lib" ? "In your library · " : "AniList · "}{metaOf(row.item)}
              </span>
            </span>
            {#if row.item.averageScore}
              <span class="inline-flex items-center gap-1 text-[10px]" style="color: var(--volt-300); font-family: var(--font-mono);">
                <Icon name="star" size={9} />
                {scoreOutOfTen(row.item.averageScore)}
              </span>
            {/if}
            <Icon name="chevronRight" size={14} class="shrink-0" />
          </button>
        {/if}
      {/each}
      </div>
    {/each}
  </div>

  {#if searching && flatRows.length === 0}
    <div class="space-y-1.5 px-2 py-2" aria-hidden="true">
      {#each Array(3) as _}
        <div class="flex items-center gap-3 p-2">
          <div class="shimmer w-9 h-[52px] rounded-md"></div>
          <div class="flex-1 space-y-2">
            <div class="shimmer h-3 w-1/2 rounded"></div>
            <div class="shimmer h-2 w-1/3 rounded"></div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
  {#snippet footer()}
    <span><b class="kbd mr-1">↑↓</b> move</span>
    <span><b class="kbd mr-1">Enter</b> open</span>
    <span><b class="kbd mr-1">Shift</b><b class="kbd">Enter</b> play</span>
    <span><b class="kbd mr-1">Esc</b> close</span>
    <span class="ml-auto hidden sm:inline">Kaminari search</span>
  {/snippet}
</Modal>

<style>
  .palette-input-row {
    transition: box-shadow 120ms ease-out;
  }

  .palette-input-row:focus-within {
    background: var(--tint-1);
    box-shadow: inset 0 0 0 2px var(--volt-400);
  }

  .palette-input,
  .palette-input:focus-visible {
    outline: none;
  }

  .palette-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 10px;
    border-radius: 10px;
    color: var(--paper-100);
    text-align: left;
    border: 1px solid transparent;
    transition: background 100ms ease-out, border-color 100ms ease-out;
  }

  .palette-row:hover,
  .palette-row.is-active {
    background: var(--accent-wash);
    border-color: var(--accent-wash-strong);
  }

  .palette-row.is-active {
    box-shadow: inset 2px 0 0 var(--volt-400);
  }

  .palette-row-icon {
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    border-radius: 8px;
    background: var(--tint-1);
    border: 1px solid var(--line);
    color: var(--text-3);
  }

  .palette-cover {
    width: 36px;
    height: 52px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    overflow: hidden;
    border-radius: 6px;
    background: var(--void-700);
    border: 1px solid var(--line);
    color: var(--text-4);
  }

  .palette-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
  }
</style>
