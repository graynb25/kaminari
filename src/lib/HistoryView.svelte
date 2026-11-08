<script lang="ts">
  import EmptyState from "./EmptyState.svelte";
  import { coverOf, displayTitle, timeAgo } from "./format";
  import type { HistoryItem, Media } from "./types";

  let {
    items,
    onOpen,
    onBrowse,
  }: {
    items: HistoryItem[];
    onOpen: (item: Media) => void;
    onBrowse: () => void;
  } = $props();
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <div class="flex flex-wrap items-baseline gap-3 mb-5">
    <h1 class="text-[28px] font-bold" style="font-family: var(--font-display);">History</h1>
    <span class="text-sm" style="color: var(--paper-500);">{items.length} {items.length === 1 ? "event" : "events"}</span>
  </div>

  {#if items.length === 0}
    <EmptyState
      icon="◷"
      title="Nothing watched yet"
      body="Mark an episode watched from a series page and it lands here with the time it happened."
      actionLabel="Browse trending"
      onAction={onBrowse}
    />
  {:else}
    <div class="rounded-2xl overflow-hidden" style="background: var(--void-900); border: 1px solid var(--line);">
      {#each items as entry, position (entry.media.id + "-" + entry.episode)}
        <button
          class="w-full flex items-center gap-3 px-3 py-2.5 text-left transition-colors"
          style="border-top: {position > 0 ? '1px solid var(--line)' : 'none'};"
          onmouseenter={(event) => (event.currentTarget.style.background = "var(--void-800)")}
          onmouseleave={(event) => (event.currentTarget.style.background = "transparent")}
          onclick={() => onOpen(entry.media)}
          aria-label="Open {displayTitle(entry.media)}, episode {entry.episode}"
        >
          <div
            class="w-10 h-14 shrink-0 rounded-md overflow-hidden"
            style="background: var(--void-700); border: 1px solid var(--line);"
          >
            {#if coverOf(entry.media)}
              <img src={coverOf(entry.media)} alt="" loading="lazy" class="w-full h-full object-cover" />
            {/if}
          </div>
          <div class="min-w-0 flex-1">
            <p class="text-sm font-semibold truncate">{displayTitle(entry.media)}</p>
            <p class="text-xs mt-0.5" style="color: var(--paper-500);">
              Episode {entry.episode}
              {#if entry.media.episodes} of {entry.media.episodes}{/if}
            </p>
          </div>
          <div class="text-right shrink-0">
            <p class="text-xs" style="color: var(--paper-300); font-family: var(--font-mono);">
              {timeAgo(entry.watchedAt)}
            </p>
            <p class="text-[10px] mt-0.5" style="color: var(--volt-300);">Watched</p>
          </div>
        </button>
      {/each}
    </div>
  {/if}
</div>
