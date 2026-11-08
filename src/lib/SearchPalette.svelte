<script lang="ts">
  import { searchAniList } from "./api";
  import { coverOf, displayTitle, metaOf } from "./format";
  import type { LibraryItem, Media } from "./types";

  let {
    open,
    library,
    onClose,
    onOpen,
    notify,
  }: {
    open: boolean;
    library: LibraryItem[];
    onClose: () => void;
    onOpen: (item: Media) => void;
    notify: (text: string, tone?: "info" | "success" | "error") => void;
  } = $props();

  const RECENT_KEY = "kaminari.recentSearches";
  const ACTIONS = [
    { id: "library", label: "Go to Library", hint: "2" },
    { id: "browse", label: "Browse trending", hint: "1" },
  ];

  let query = $state("");
  let results = $state<Media[]>([]);
  let lastGood = $state<Media[]>([]);
  let searching = $state(false);
  let error = $state<string | null>(null);
  let rateLimited = $state(false);
  let requestId = $state(0);
  let recent = $state<string[]>(loadRecent());
  let active = $state(0);
  let input: HTMLInputElement | null = $state(null);
  let dialog: HTMLDivElement | null = $state(null);

  type Row =
    | { kind: "lib"; item: Media }
    | { kind: "net"; item: Media }
    | { kind: "action"; id: string; label: string };

  const libHits = $derived(
    query.trim().length >= 2
      ? library
          .filter((entry) => displayTitle(entry.media).toLowerCase().includes(query.trim().toLowerCase()))
          .slice(0, 3)
          .map((entry) => entry.media)
      : [],
  );

  const rows = $derived<Row[]>([
    ...libHits.map((item): Row => ({ kind: "lib", item })),
    ...results.map((item): Row => ({ kind: "net", item })),
    ...(query.trim().length < 2 ? ACTIONS.map((a): Row => ({ kind: "action", id: a.id, label: a.label })) : []),
  ]);

  function loadRecent(): string[] {
    try {
      const raw = localStorage.getItem(RECENT_KEY);
      const parsed: unknown = raw ? JSON.parse(raw) : [];
      return Array.isArray(parsed) ? parsed.filter((x): x is string => typeof x === "string").slice(0, 8) : [];
    } catch {
      return [];
    }
  }

  function saveRecent(term: string) {
    const next = [term, ...recent.filter((r) => r !== term)].slice(0, 8);
    recent = next;
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(next));
    } catch {
      // Private mode or blocked storage. Recent list just stays in memory.
    }
  }

  function reset() {
    query = "";
    results = [];
    error = null;
    rateLimited = false;
    active = 0;
  }

  // Focus the input whenever the palette opens. Parent restores focus on close.
  $effect(() => {
    if (open) {
      reset();
      requestAnimationFrame(() => input?.focus());
    }
  });

  // Debounced live search. Stale responses are dropped by request id.
  $effect(() => {
    const term = query.trim();
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
        } finally {
          if (id === requestId) searching = false;
        }
      })();
    }, 200);
    return () => window.clearTimeout(timer);
  });

  // Keep Tab cycling inside the dialog. Esc always closes, handled above.
  function trapTab(event: KeyboardEvent) {
    if (event.key !== "Tab" || !dialog) return;
    const items = [...dialog.querySelectorAll("button, input")].filter(
      (el) => (el as HTMLElement).tabIndex >= 0,
    ) as HTMLElement[];
    if (items.length === 0) return;
    const first = items[0];
    const last = items[items.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function choose(row: Row | undefined) {
    if (!row) return;
    if (row.kind === "action") {
      notify(row.id === "library" ? "Library opens on key 2." : "Trending is already on screen.", "info");
      onClose();
      return;
    }
    if (query.trim().length >= 2) saveRecent(query.trim());
    onOpen(row.item);
    onClose();
  }

  function onKeys(event: KeyboardEvent) {    if (event.key === "ArrowDown") {
      event.preventDefault();
      active = rows.length ? (active + 1) % rows.length : 0;
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      active = rows.length ? (active - 1 + rows.length) % rows.length : 0;
    } else if (event.key === "Enter") {
      event.preventDefault();
      choose(rows[active]);
    } else if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
  }
</script>

{#if open}
  <div
    class="fixed inset-0 z-50 flex justify-center px-4"
    style="background: rgb(4 4 8 / 0.6); padding-top: 15vh;"
    onclick={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}
    role="presentation"
  >
    <div
      bind:this={dialog}
      onkeydown={trapTab}
      tabindex={-1}
      class="modal-in glass w-full max-w-[640px] h-fit rounded-[20px] overflow-hidden"
      style="border: 1px solid var(--line); box-shadow: var(--shadow-pop);"
      role="dialog"
      aria-modal="true"
      aria-label="Search"
    >
      <div class="flex items-center gap-2 px-4" style="border-bottom: 1px solid var(--line);">
        <span aria-hidden="true" style="color: var(--paper-500);">⌕</span>
        <input
          bind:this={input}
          bind:value={query}
          onkeydown={onKeys}
          placeholder="Search AniList, two or more chars"
          class="flex-1 h-12 bg-transparent text-sm outline-none"
          style="color: var(--paper-50);"
          aria-label="Search AniList"
          autocomplete="off"
          spellcheck={false}
        />
        {#if searching}
          <span class="text-xs" style="color: var(--paper-500);">Searching</span>
        {/if}
        <button
          class="text-xs px-2 py-1 rounded-md"
          style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
          onclick={onClose}
          aria-label="Close search"
        >
          Esc
        </button>
      </div>

      <div class="max-h-[46vh] overflow-y-auto p-2">
        {#if rateLimited}
          <div
            class="m-2 px-3 py-2 rounded-lg text-sm"
            style="background: rgb(251 191 36 / 0.1); border: 1px solid rgb(251 191 36 / 0.35); color: var(--paper-50);"
            role="status"
          >
            AniList is rate limiting. Showing last good results. Try again in a few seconds.
          </div>
        {:else if error}
          <div
            class="m-2 px-3 py-2 rounded-lg text-sm"
            style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
            role="alert"
          >
            Search failed: {error}
          </div>
        {/if}

        {#if query.trim().length < 2}
          {#if recent.length > 0}
            <p class="px-3 pt-2 pb-1 text-[11px] font-semibold uppercase tracking-wider" style="color: var(--paper-500);">
              Recent
            </p>
            {#each recent as term}
              <button
                class="w-full text-left px-3 py-2 rounded-lg text-sm"
                style="color: var(--paper-300);"
                onclick={() => {
                  query = term;
                }}
              >
                ↺ {term}
              </button>
            {/each}
          {:else}
            <p class="px-3 py-4 text-sm" style="color: var(--paper-500);">
              Type two or more characters. Library matches show first, AniList next.
            </p>
          {/if}
        {:else if rows.length === 0 && !searching}
          <p class="px-3 py-4 text-sm" style="color: var(--paper-500);">
            No matches yet. Try romaji or a shorter title.
            {#if lastGood.length > 0}Last results are kept below.{/if}
          </p>
        {/if}

        {#each rows as row, position (row.kind + (row.kind === "action" ? row.id : row.item.id))}
          {@const isActive = position === active}
          {#if row.kind === "action"}
            <button
              class="w-full text-left px-3 py-2 rounded-lg text-sm"
              style="background: {isActive ? 'var(--void-700)' : 'transparent'}; color: var(--paper-50);"
              onmouseenter={() => (active = position)}
              onclick={() => choose(row)}
            >
              → {row.label}
            </button>
          {:else}
            {@const art = coverOf(row.item)}
            <button
              class="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-left"
              style="background: {isActive ? 'var(--void-700)' : 'transparent'};"
              onmouseenter={() => (active = position)}
              onclick={() => choose(row)}
              aria-label="Open {displayTitle(row.item)}"
            >
              <span
                class="w-10 h-[60px] rounded-md overflow-hidden shrink-0"
                style="background: {row.item.coverImage?.color ?? 'var(--void-700)'};"
              >
                {#if art}
                  <img src={art} alt="" loading="lazy" class="w-full h-full object-cover" />
                {/if}
              </span>
              <span class="min-w-0">
                <span class="block text-sm font-semibold truncate">{displayTitle(row.item)}</span>
                <span class="block text-xs mt-0.5" style="color: var(--paper-500);">
                  {row.kind === "lib" ? "In your library · " : ""}{metaOf(row.item)}
                </span>
              </span>
            </button>
          {/if}
        {/each}
      </div>

      <div
        class="flex gap-4 px-4 py-2.5 text-[11px]"
        style="border-top: 1px solid var(--line); color: var(--paper-500);"
      >
        <span><b>↑↓</b> move</span>
        <span><b>Enter</b> open</span>
        <span><b>Esc</b> close</span>
      </div>
    </div>
  </div>
{/if}
