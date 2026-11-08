<script lang="ts">
  import EmptyState from "./EmptyState.svelte";
  import PosterCard from "./PosterCard.svelte";
  import type { LibraryItem, Media } from "./types";

  let {
    items,
    onOpen,
    onToggleLibrary,
    onPlay,
    onBrowse,
  }: {
    items: LibraryItem[];
    onOpen: (item: Media) => void;
    onToggleLibrary: (item: Media) => void;
    onPlay: (item: Media) => void;
    onBrowse: () => void;
  } = $props();
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <div class="flex items-baseline gap-3 mb-5">
    <h1 class="text-[28px] font-bold" style="font-family: var(--font-display);">Your library</h1>
    <span class="text-sm" style="color: var(--paper-500);">{items.length} titles</span>
  </div>

  {#if items.length === 0}
    <EmptyState
      icon="▤"
      title="Your library is empty"
      body="Add a title from Browse and it will live here, with progress and history beside it in P1."
      actionLabel="Browse trending"
      onAction={onBrowse}
    />
  {:else}
    <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
      {#each items as entry (entry.media.id)}
        <div>
          <PosterCard
            item={entry.media}
            inLibrary={true}
            onOpen={onOpen}
            onToggleLibrary={onToggleLibrary}
            onPlay={onPlay}
          />
          <div class="flex items-center gap-2 mt-2 px-0.5">
            <span class="chip text-[11px] px-2 py-0.5 capitalize">{entry.status}</span>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
