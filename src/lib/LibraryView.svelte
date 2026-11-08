<script lang="ts">
  import EmptyState from "./EmptyState.svelte";
  import PosterCard from "./PosterCard.svelte";
  import { displayTitle } from "./format";
  import type { LibraryItem, Media } from "./types";

  let {
    items,
    onOpen,
    onToggleLibrary,
    onPlay,
    onBrowse,
    onSetStatus,
  }: {
    items: LibraryItem[];
    onOpen: (item: Media) => void;
    onToggleLibrary: (item: Media) => void;
    onPlay: (item: Media) => void;
    onBrowse: () => void;
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

  function label(status: string): string {
    return status.charAt(0).toUpperCase() + status.slice(1);
  }
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <div class="flex flex-wrap items-baseline gap-3 mb-5">
    <h1 class="text-[28px] font-bold" style="font-family: var(--font-display);">Your library</h1>
    <span class="text-sm" style="color: var(--paper-500);">{items.length} {items.length === 1 ? "title" : "titles"}</span>
    <div class="ml-auto flex items-center gap-2">
      <input
        bind:value={query}
        placeholder="Filter by title"
        class="h-8 w-56 rounded-lg px-3 text-sm outline-none"
        style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-50);"
        aria-label="Filter library by title"
      />
    </div>
  </div>

  {#if items.length === 0}
    <EmptyState
      icon="▤"
      title="Your library is empty"
      body="Add a title from Browse and it will live here, with progress and history beside it."
      actionLabel="Browse trending"
      onAction={onBrowse}
    />
  {:else}
    <div class="flex flex-wrap gap-1.5 mb-6" role="group" aria-label="Filter by status">
      {#each statuses as status}
        {@const count = status === "all" ? items.length : items.filter((entry) => entry.status === status).length}
        <button
          class="px-3 py-1.5 rounded-full text-xs font-semibold"
          style="background: {statusFilter === status ? 'var(--volt-500)' : 'var(--void-800)'}; border: 1px solid {statusFilter === status ? 'var(--volt-500)' : 'var(--line)'}; color: {statusFilter === status ? 'var(--paper-50)' : 'var(--paper-300)'};"
          aria-pressed={statusFilter === status}
          onclick={() => (statusFilter = status)}
        >
          {status === "all" ? "All" : label(status)}
          <span style="color: {statusFilter === status ? 'rgb(244 243 255 / 0.72)' : 'var(--paper-500)'}; font-family: var(--font-mono);">
            {count}
          </span>
        </button>
      {/each}
    </div>

    {#if filtered.length === 0}
      <div
        class="rounded-2xl px-5 py-6 text-sm"
        style="background: var(--void-900); border: 1px solid var(--line); color: var(--paper-300);"
      >
        No titles match this filter. Clear it or switch status.
      </div>
    {:else}
      <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
        {#each filtered as entry (entry.media.id)}
          <div>
            <PosterCard
              item={entry.media}
              inLibrary={true}
              onOpen={onOpen}
              onToggleLibrary={onToggleLibrary}
              onPlay={onPlay}
            />
            <div class="flex items-center gap-1.5 mt-2 px-0.5">
              <select
                class="h-7 rounded-md px-1.5 text-[11px] font-semibold outline-none"
                style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
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
                <span class="text-[11px] ml-auto" style="color: var(--paper-500); font-family: var(--font-mono);">
                  {entry.watchedCount} / {entry.media.episodes}
                </span>
              {/if}
            </div>
            {#if entry.media.episodes}
              <div class="h-0.5 rounded-full mt-1.5 mx-0.5 overflow-hidden" style="background: var(--void-700);" aria-hidden="true">
                <div
                  class="h-full"
                  style="width: {Math.min(100, (entry.watchedCount / entry.media.episodes) * 100)}%; background: var(--volt-400);"
                ></div>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>
