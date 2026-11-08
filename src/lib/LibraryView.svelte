<script lang="ts">
  import EmptyState from "./EmptyState.svelte";
  import Icon from "./Icon.svelte";
  import PosterCard from "./PosterCard.svelte";
  import { displayTitle } from "./format";
  import type { LibraryItem, Media, SourceLibraryItem } from "./types";

  let {
    items,
    sourceItems,
    error = null,
    onRetry,
    onOpen,
    onOpenSource,
    onResumeSource,
    onToggleLibrary,
    onPlay,
    onBrowse,
    onOpenSearch,
    onOpenSources,
    onSetStatus,
  }: {
    items: LibraryItem[];
    sourceItems: SourceLibraryItem[];
    error?: string | null;
    onRetry?: () => void;
    onOpen: (item: Media) => void;
    onOpenSource: (item: SourceLibraryItem) => void;
    onResumeSource: (item: SourceLibraryItem) => void;
    onToggleLibrary: (item: Media) => void;
    onPlay: (item: Media) => void;
    onBrowse: () => void;
    onOpenSearch: () => void;
    onOpenSources: () => void;
    onSetStatus: (item: Media, status: string) => void;
  } = $props();

  const statuses = ["all", "watching", "planning", "completed", "paused", "dropped"] as const;
  let statusFilter = $state<(typeof statuses)[number]>("all");
  let query = $state("");

  const filtered = $derived.by(() => {
    const text = query.trim().toLowerCase();
    return items.filter((entry) => {
      if (statusFilter !== "all" && entry.status !== statusFilter) return false;
      if (!text) return true;
      return displayTitle(entry.media).toLowerCase().includes(text);
    });
  });

  const filteredSource = $derived.by(() => {
    const text = query.trim().toLowerCase();
    if (!text) return sourceItems;
    return sourceItems.filter((entry) => entry.title.toLowerCase().includes(text));
  });

  const watchingCount = $derived(items.filter((entry) => entry.status === "watching").length);
  const completedCount = $derived(items.filter((entry) => entry.status === "completed").length);
  const watchedTotal = $derived(items.reduce((sum, entry) => sum + (entry.watchedCount || 0), 0));
  const totalTitles = $derived(items.length + sourceItems.length);
  const summary = $derived.by(() => {
    if (error && totalTitles === 0) return "The local library could not be read.";
    if (totalTitles === 0) return "Saved titles, progress and source runs live here.";
    if (items.length > 0 && sourceItems.length > 0) {
      return `${items.length} AniList ${items.length === 1 ? "title" : "titles"} and ${sourceItems.length} source ${sourceItems.length === 1 ? "run" : "runs"}.`;
    }
    if (items.length > 0) return `${items.length} AniList ${items.length === 1 ? "title" : "titles"} tracked on this machine.`;
    return `${sourceItems.length} source ${sourceItems.length === 1 ? "run" : "runs"} tracked on this machine.`;
  });

  function label(status: string): string {
    return status.charAt(0).toUpperCase() + status.slice(1);
  }

  function statusColor(status: string): string {
    if (status === "watching") return "var(--success)";
    if (status === "completed") return "var(--volt-400)";
    if (status === "planning") return "var(--info)";
    if (status === "paused") return "var(--warning)";
    if (status === "dropped") return "var(--danger)";
    return "var(--paper-500)";
  }

  function formatTime(value?: number | null): string {
    if (!value || value <= 0) return "0:00";
    const minutes = Math.floor(value / 60);
    const seconds = Math.floor(value % 60);
    return `${minutes}:${String(seconds).padStart(2, "0")}`;
  }

  function clearFilters() {
    query = "";
    statusFilter = "all";
  }
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <div class="flex flex-wrap items-start gap-4 mb-6">
    <div class="min-w-0">
      <p class="eyebrow">Collection</p>
      <h1 class="page-title mt-1">Your library</h1>
      <p class="page-sub mt-1">{summary}</p>
    </div>
    <div class="ml-auto flex items-center gap-2">
      {#if items.length > 0}
        <label class="relative block">
          <span class="sr-only">Filter library by title</span>
          <span class="absolute left-3 top-1/2 -translate-y-1/2" style="color: var(--paper-450);">
            <Icon name="search" size={14} />
          </span>
          <input
            bind:value={query}
            placeholder="Filter by title"
            class="h-9 w-56 rounded-xl pl-9 pr-3 text-sm outline-none"
            style="background: rgb(23 23 38 / 0.72); border: 1px solid var(--line); color: var(--paper-50);"
            aria-label="Filter library by title"
          />
        </label>
      {:else}
        <button class="btn-ghost px-3.5 py-2 text-xs" onclick={onOpenSearch}>
          <Icon name="search" size={14} />
          Search titles
        </button>
      {/if}
    </div>
  </div>

  {#if error && totalTitles > 0}
    <div class="surface px-4 py-3 mb-4 flex flex-col sm:flex-row sm:items-center gap-2 text-sm" role="alert" style="color: var(--paper-300);">
      <Icon name="alert" size={15} />
      <span class="flex-1">Library refresh failed: {error}</span>
      {#if onRetry}
        <button class="btn-ghost px-3 py-1.5 text-xs shrink-0" onclick={onRetry}>Retry</button>
      {/if}
    </div>
  {/if}

  {#if error && totalTitles === 0}
    <div class="surface p-5 max-w-xl">
      <div class="flex items-start gap-3">
        <span class="w-9 h-9 grid place-items-center rounded-lg shrink-0" style="background: rgb(251 113 133 / 0.1); color: var(--danger);">
          <Icon name="alert" size={17} />
        </span>
        <div class="min-w-0 flex-1">
          <h2 class="text-sm font-semibold">Library did not load</h2>
          <p class="text-xs mt-1 leading-relaxed" style="color: var(--paper-300);">{error}</p>
        </div>
        {#if onRetry}
          <button class="btn-ghost px-3 py-1.5 text-xs shrink-0" onclick={onRetry}>Retry</button>
        {/if}
      </div>
    </div>
  {:else if totalTitles === 0}
    <EmptyState
      icon="library"
      title="Your library is waiting"
      body="Save a title from Browse or a source, and it will live here with status, progress and history."
      actionLabel="Browse trending"
      onAction={onBrowse}
      secondaryLabel="Search titles"
      onSecondary={onOpenSearch}
    />
  {:else}
    <div class="grid grid-cols-2 md:grid-cols-4 gap-3 mb-6">
      <div class="stat-card">
        <p class="stat-label">Titles</p>
        <p class="stat-value">{totalTitles}</p>
      </div>
      <div class="stat-card">
        <p class="stat-label">Watching</p>
        <p class="stat-value">{watchingCount}</p>
      </div>
      <div class="stat-card">
        <p class="stat-label">Episodes watched</p>
        <p class="stat-value">{watchedTotal}</p>
      </div>
      <div class="stat-card">
        <p class="stat-label">Completed</p>
        <p class="stat-value">{completedCount}</p>
      </div>
    </div>

    {#if items.length > 0}
      <div class="flex flex-wrap items-center gap-2 mb-6">
        <div class="flex flex-wrap gap-1.5" role="group" aria-label="Filter by status">
          {#each statuses as status}
            {@const count = status === "all" ? items.length : items.filter((entry) => entry.status === status).length}
            <button
              class="px-3 py-1.5 rounded-full text-[11px] font-semibold transition-colors"
              style="background: {statusFilter === status ? 'linear-gradient(180deg, var(--volt-400), var(--volt-500))' : 'rgb(23 23 38 / 0.72)'}; border: 1px solid {statusFilter === status ? 'rgb(255 255 255 / 0.18)' : 'var(--line)'}; color: {statusFilter === status ? '#fff' : 'var(--paper-300)'};"
              aria-pressed={statusFilter === status}
              onclick={() => (statusFilter = status)}
            >
              {status === "all" ? "All" : label(status)}
              <span style="color: {statusFilter === status ? 'rgb(255 255 255 / 0.72)' : 'var(--paper-500)'}; font-family: var(--font-mono);">
                {count}
              </span>
            </button>
          {/each}
        </div>
        <span class="ml-auto text-[11px]" style="color: var(--paper-450); font-family: var(--font-mono);">
          {filtered.length} of {items.length}
        </span>
      </div>

      {#if filtered.length === 0}
        <EmptyState
          compact
          icon="filter"
          title="No titles match"
          body={query.trim() ? `No results for "${query.trim()}". Try a shorter title or another status.` : "No titles carry this status yet."}
          actionLabel="Clear filters"
          actionIcon="filter"
          onAction={clearFilters}
        />
      {:else}
        <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6 gap-5">
          {#each filtered as entry (entry.media.id)}
            <div>
              <PosterCard
                item={entry.media}
                inLibrary={true}
                progress={entry.media.episodes ? { watched: entry.watchedCount, total: entry.media.episodes } : null}
                onOpen={onOpen}
                onToggleLibrary={onToggleLibrary}
                onPlay={onPlay}
              />
              <div class="flex items-center gap-1.5 mt-2 px-0.5">
                <span class="w-1.5 h-1.5 rounded-full shrink-0" style="background: {statusColor(entry.status)};" aria-hidden="true"></span>
                <select
                  class="h-7 rounded-md px-1.5 text-[11px] font-semibold outline-none"
                  style="background: rgb(23 23 38 / 0.72); border: 1px solid var(--line); color: var(--paper-300);"
                  value={entry.status}
                  onchange={(event) =>
                    onSetStatus(entry.media, (event.currentTarget as HTMLSelectElement).value)}
                  aria-label="Status for {displayTitle(entry.media)}"
                >
                  <option value="watching">Watching</option>
                  <option value="planning">Planning</option>
                  <option value="completed">Completed</option>
                  <option value="paused">Paused</option>
                  <option value="dropped">Dropped</option>
                </select>
                {#if entry.media.episodes}
                  <span class="text-[10px] ml-auto" style="color: var(--paper-450); font-family: var(--font-mono);">
                    {entry.watchedCount} / {entry.media.episodes}
                  </span>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {:else}
      <div class="start-callout mb-6">
        <span class="w-10 h-10 grid place-items-center rounded-xl shrink-0" style="background: rgb(108 107 245 / 0.14); border: 1px solid var(--line-volt); color: var(--volt-300);">
          <Icon name="sparkles" size={18} />
        </span>
        <div class="min-w-0 flex-1">
          <p class="text-sm font-semibold">The AniList library is empty</p>
          <p class="text-xs mt-0.5" style="color: var(--paper-400);">Save a title from Browse and it lands here with progress and status.</p>
        </div>
        <button class="btn-ghost px-3.5 py-2 text-xs shrink-0" onclick={onBrowse}>
          Browse
          <Icon name="arrowRight" size={13} />
        </button>
      </div>
    {/if}

    {#if filteredSource.length > 0}
      <section class={items.length > 0 ? "mt-10" : ""} aria-label="Source library">
        <div class="flex items-center gap-3 mb-4">
          <div>
            <p class="eyebrow">Source runs</p>
            <h2 class="shelf-title mt-1">Source library</h2>
          </div>
          <span class="shelf-count">{filteredSource.length}</span>
          <div class="flex-1"></div>
          <button class="btn-ghost px-3 py-1.5 text-[11px]" onclick={onOpenSources}>
            <Icon name="package" size={13} />
            Manage sources
          </button>
        </div>
        <div class="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-4">
          {#each filteredSource as item (item.extensionId + "|" + item.showId)}
            <article class="library-source-card">
              <button
                class="library-source-art"
                onclick={() => onOpenSource(item)}
                aria-label="Open {item.title} from {item.extensionName}"
              >
                {#if item.cover}
                  <img src={item.cover} alt="" loading="lazy" decoding="async" />
                {:else}
                  <div class="w-full h-full" style="background: radial-gradient(circle at 30% 20%, rgb(139 138 255 / 0.2), transparent 60%), var(--void-800);"></div>
                {/if}
                <span class="library-source-scrim" aria-hidden="true"></span>
                <span class="absolute top-2.5 left-2.5 inline-flex items-center h-6 px-2 rounded-full text-[9px] font-semibold tracking-wider" style="background: rgb(8 8 15 / 0.66); border: 1px solid var(--line); color: var(--paper-300); backdrop-filter: blur(10px);">
                  {item.extensionName}
                </span>
                {#if item.lastEpisode != null}
                  <span class="absolute top-2.5 right-2.5 inline-flex items-center h-6 px-2 rounded-full text-[10px] font-semibold" style="background: rgb(8 8 15 / 0.66); border: 1px solid var(--line); color: var(--volt-200); font-family: var(--font-mono); backdrop-filter: blur(10px);">
                    EP {item.lastEpisode}
                  </span>
                {/if}
                <span class="absolute right-3 bottom-3 h-9 w-9 grid place-items-center rounded-full" style="background: linear-gradient(180deg, var(--volt-400), var(--volt-500)); color: #fff; box-shadow: 0 6px 20px rgb(108 107 245 / 0.4);" aria-hidden="true">
                  <Icon name="play" size={14} />
                </span>
                <span class="absolute left-3 right-14 bottom-3 block">
                  <span class="block text-sm font-semibold leading-snug line-clamp-1">{item.title}</span>
                  <span class="block text-[10px] mt-0.5" style="color: var(--paper-300); font-family: var(--font-mono);">
                    {item.watchedCount} watched{item.year ? ` · ${item.year}` : ""}
                  </span>
                </span>
                {#if item.lastDuration && item.lastPosition && item.lastPosition < item.lastDuration * 0.95}
                  <span class="absolute left-0 right-0 bottom-0 h-[3px]" style="background: rgb(0 0 0 / 0.42);" aria-hidden="true">
                    <span class="block h-full" style="width: {Math.min(100, (item.lastPosition / item.lastDuration) * 100)}%; background: var(--volt-400); box-shadow: 0 0 10px var(--volt-400);"></span>
                  </span>
                {/if}
              </button>
              <div class="p-3 flex items-center gap-2">
                <div class="min-w-0 flex-1">
                  {#if item.lastEpisode != null}
                    <p class="text-[11px]" style="color: var(--paper-300);">
                      Episode {item.lastEpisode}
                      {#if item.lastDuration && item.lastPosition}
                        <span style="color: var(--paper-450); font-family: var(--font-mono);">
                          · {formatTime(item.lastPosition)} / {formatTime(item.lastDuration)}
                        </span>
                      {/if}
                    </p>
                  {:else}
                    <p class="text-[11px]" style="color: var(--paper-450);">Not started</p>
                  {/if}
                </div>
                <button class="btn-primary px-3 py-1.5 text-[11px] shrink-0" onclick={() => (item.lastEpisodeId ? onResumeSource(item) : onOpenSource(item))}>
                  {item.lastPosition && item.lastDuration && item.lastPosition / item.lastDuration >= 0.95
                    ? "Rewatch"
                    : item.lastPosition
                      ? "Resume"
                      : "Open"}
                </button>
              </div>
            </article>
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</div>
