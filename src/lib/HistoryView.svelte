<script lang="ts">
  import EmptyState from "./EmptyState.svelte";
  import Icon from "./Icon.svelte";
  import Skeleton from "./Skeleton.svelte";
  import { coverOf, displayTitle, timeAgo } from "./format";
  import type { HistoryItem, Media } from "./types";

  let {
    items,
    loading = false,
    error = null,
    onRetry,
    onOpen,
    onBrowse,
  }: {
    items: HistoryItem[];
    loading?: boolean;
    error?: string | null;
    onRetry?: () => void;
    onOpen: (item: Media) => void;
    onBrowse: () => void;
  } = $props();

  function dayKey(unixSeconds: number): string {
    const date = new Date(unixSeconds * 1000);
    return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
  }

  function dayLabel(unixSeconds: number): string {
    const date = new Date(unixSeconds * 1000);
    const today = new Date();
    const yesterday = new Date();
    yesterday.setDate(today.getDate() - 1);
    if (dayKey(unixSeconds) === dayKey(today.getTime() / 1000)) return "Today";
    if (dayKey(unixSeconds) === dayKey(yesterday.getTime() / 1000)) return "Yesterday";
    return date.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      year: date.getFullYear() === today.getFullYear() ? undefined : "numeric",
    });
  }

  const groups = $derived.by(() => {
    const map = new Map<string, HistoryItem[]>();
    for (const entry of items) {
      const key = dayKey(entry.watchedAt);
      const list = map.get(key);
      if (list) list.push(entry);
      else map.set(key, [entry]);
    }
    return Array.from(map.values()).map((entries) => ({
      label: dayLabel(entries[0].watchedAt),
      entries,
    }));
  });

  const uniqueTitles = $derived(new Set(items.map((entry) => entry.media.id)).size);
  const latest = $derived(items.length > 0 ? timeAgo(items[0].watchedAt) : "No activity");
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <div class="flex flex-wrap items-start gap-4 mb-6">
    <div>
      <p class="eyebrow">Activity</p>
      <h1 class="page-title mt-1">History</h1>
      <p class="page-sub mt-1">Every watched episode, newest first, stored on this machine.</p>
    </div>
    {#if items.length > 0}
      <div class="ml-auto grid grid-cols-3 gap-3">
        <div class="stat-card">
          <p class="stat-label">Events</p>
          <p class="stat-value">{items.length}</p>
        </div>
        <div class="stat-card">
          <p class="stat-label">Titles</p>
          <p class="stat-value">{uniqueTitles}</p>
        </div>
        <div class="stat-card">
          <p class="stat-label">Latest</p>
          <p class="stat-value text-base" style="font-size: 15px;">{latest}</p>
        </div>
      </div>
    {/if}
  </div>

  {#if loading && items.length === 0}
    <Skeleton kind="list" />
  {:else if error && items.length === 0}
    <div class="surface p-5 max-w-xl">
      <div class="flex items-start gap-3">
        <span class="w-9 h-9 grid place-items-center rounded-lg shrink-0" style="background: rgb(251 113 133 / 0.1); color: var(--danger);">
          <Icon name="alert" size={17} />
        </span>
        <div class="min-w-0 flex-1">
          <h2 class="text-sm font-semibold">History did not load</h2>
          <p class="text-xs mt-1 leading-relaxed" style="color: var(--paper-300);">{error}</p>
        </div>
        {#if onRetry}
          <button class="btn-ghost px-3 py-1.5 text-xs shrink-0" onclick={onRetry}>Retry</button>
        {/if}
      </div>
    </div>
  {:else if items.length === 0}
    <EmptyState
      icon="history"
      title="Nothing watched yet"
      body="Mark an episode watched from a series page and it lands here with the time it happened."
      actionLabel="Browse trending"
      onAction={onBrowse}
    />
  {:else}
    <div class="space-y-7">
      {#each groups as group (group.label)}
        <section aria-label={group.label}>
          <div class="flex items-center gap-3 mb-2.5">
            <h2 class="text-[11px] font-semibold uppercase tracking-[0.16em]" style="color: var(--paper-450);">{group.label}</h2>
            <span class="flex-1 h-px" style="background: var(--line);"></span>
            <span class="text-[10px]" style="color: var(--paper-450); font-family: var(--font-mono);">{group.entries.length}</span>
          </div>
          <div class="surface overflow-hidden">
            {#each group.entries as entry, position (entry.media.id + "-" + entry.episode)}
              <button
                class="history-row w-full flex items-center gap-3 px-3 py-2.5 text-left"
                style="border-top: {position > 0 ? '1px solid var(--line)' : 'none'};"
                onclick={() => onOpen(entry.media)}
                aria-label="Open {displayTitle(entry.media)}, episode {entry.episode}"
              >
                <div class="w-10 h-14 shrink-0 rounded-lg overflow-hidden" style="background: var(--void-700); border: 1px solid var(--line);">
                  {#if coverOf(entry.media)}
                    <img src={coverOf(entry.media)} alt="" loading="lazy" decoding="async" class="w-full h-full object-cover" />
                  {/if}
                </div>
                <div class="min-w-0 flex-1">
                  <p class="text-sm font-semibold truncate">{displayTitle(entry.media)}</p>
                  <p class="text-xs mt-0.5" style="color: var(--paper-450);">
                    Episode {entry.episode}
                    {#if entry.media.episodes} of {entry.media.episodes}{/if}
                  </p>
                </div>
                <span class="chip chip-live text-[10px] px-2 py-0.5 hidden sm:inline-flex">
                  <Icon name="check" size={10} />
                  Watched
                </span>
                <div class="text-right shrink-0 min-w-[72px]">
                  <p class="text-[11px]" style="color: var(--paper-300); font-family: var(--font-mono);">
                    {timeAgo(entry.watchedAt)}
                  </p>
                </div>
                <span style="color: var(--paper-450);" aria-hidden="true">
                  <Icon name="chevronRight" size={15} />
                </span>
              </button>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  {/if}
</div>

<style>
  .history-row {
    transition: background var(--dur-fast) var(--ease-out);
  }

  .history-row:hover {
    background: rgb(255 255 255 / 0.045);
  }

  .history-row:focus-visible {
    outline: 2px solid var(--volt-400);
    outline-offset: -2px;
  }
</style>
