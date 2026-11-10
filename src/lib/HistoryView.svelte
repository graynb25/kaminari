<script lang="ts">
  import Artwork from "./Artwork.svelte";
  import EmptyState from "./EmptyState.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import PageHeader from "./PageHeader.svelte";
  import Skeleton from "./Skeleton.svelte";
  import { coverOf, displayTitle, timeAgo } from "./format";
  import type { HistoryItem, Media, SourceHistoryItem } from "./types";

  type HistoryRow =
    | { kind: "anilist"; at: number; item: HistoryItem }
    | { kind: "source"; at: number; item: SourceHistoryItem };
  type RangeKey = "hour" | "today" | "yesterday" | "all";

  const ranges: { key: RangeKey; label: string }[] = [
    { key: "hour", label: "The last hour" },
    { key: "today", label: "Today" },
    { key: "yesterday", label: "Today and yesterday" },
    { key: "all", label: "All time" },
  ];

  let {
    items,
    sourceItems = [],
    loading = false,
    error = null,
    onRetry,
    onOpen,
    onOpenSource,
    onBrowse,
    onClearHistory,
    onDeleteHistory,
    onDeleteSourceHistory,
  }: {
    items: HistoryItem[];
    sourceItems?: SourceHistoryItem[];
    loading?: boolean;
    error?: string | null;
    onRetry?: () => void;
    onOpen: (item: Media) => void;
    onOpenSource: (item: SourceHistoryItem) => void;
    onBrowse: () => void;
    onClearHistory?: () => Promise<void> | void;
    onDeleteHistory?: (item: HistoryItem) => Promise<void> | void;
    onDeleteSourceHistory?: (item: SourceHistoryItem) => Promise<void> | void;
  } = $props();

  let range = $state<RangeKey>("all");
  let rangeOpen = $state(false);
  let clearOpen = $state(false);
  let clearing = $state(false);
  let deletingKey = $state<string | null>(null);
  let rangeWrap = $state<HTMLDivElement | null>(null);

  function rowKey(row: HistoryRow): string {
    return row.kind === "anilist"
      ? `a:${row.item.media.id}:${row.item.episode}`
      : `s:${row.item.extensionId}:${row.item.showId}:${row.item.episodeId}`;
  }

  function startOfDayDaysAgo(days: number): number {
    const date = new Date();
    date.setDate(date.getDate() - days);
    date.setHours(0, 0, 0, 0);
    return Math.floor(date.getTime() / 1000);
  }

  function rangeCutoff(key: RangeKey): number {
    if (key === "hour") return Math.floor(Date.now() / 1000) - 3600;
    if (key === "today") return startOfDayDaysAgo(0);
    if (key === "yesterday") return startOfDayDaysAgo(1);
    return 0;
  }

  const rangeLabel = $derived(ranges.find((entry) => entry.key === range)?.label ?? "All time");

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

  const merged = $derived.by(() => {
    const rows: HistoryRow[] = [
      ...items.map((item) => ({ kind: "anilist" as const, at: item.watchedAt, item })),
      ...sourceItems.map((item) => ({ kind: "source" as const, at: item.updatedAt, item })),
    ];
    return rows.sort((a, b) => b.at - a.at);
  });

  const filtered = $derived.by(() => {
    const cutoff = rangeCutoff(range);
    return cutoff === 0 ? merged : merged.filter((row) => row.at >= cutoff);
  });

  const groups = $derived.by(() => {
    const map = new Map<string, HistoryRow[]>();
    for (const row of filtered) {
      const key = dayKey(row.at);
      const list = map.get(key);
      if (list) list.push(row);
      else map.set(key, [row]);
    }
    return Array.from(map.values()).map((entries) => ({
      label: dayLabel(entries[0].at),
      entries,
    }));
  });

  const uniqueTitles = $derived(
    new Set(
      filtered.map((row) =>
        row.kind === "anilist"
          ? `a:${row.item.media.id}`
          : `s:${row.item.extensionId}:${row.item.showId}`,
      ),
    ).size,
  );
  const latest = $derived(filtered.length > 0 ? timeAgo(filtered[0].at) : "No activity");
  const summary = $derived.by(() => {
    if (filtered.length === 0) return "No events in this range";
    const eventLabel = filtered.length === 1 ? "event" : "events";
    const titleLabel = uniqueTitles === 1 ? "title" : "titles";
    return `${filtered.length} ${eventLabel} · ${uniqueTitles} ${titleLabel} · latest ${latest}`;
  });
  const emptyRangeLabel = $derived(
    range === "hour"
      ? "in the last hour"
      : range === "today"
        ? "today"
        : range === "yesterday"
          ? "today or yesterday"
          : "in this range",
  );

  function selectRange(key: RangeKey) {
    range = key;
    rangeOpen = false;
  }

  function openClear() {
    rangeOpen = false;
    clearOpen = true;
  }

  function closeClear() {
    if (!clearing) clearOpen = false;
  }

  async function clearHistory() {
    if (clearing || !onClearHistory) return;
    clearing = true;
    try {
      await onClearHistory();
      clearOpen = false;
    } catch {
      // The parent callback reports failures.
    } finally {
      clearing = false;
    }
  }

  async function deleteRow(row: HistoryRow) {
    if (deletingKey !== null) return;
    const key = rowKey(row);
    deletingKey = key;
    try {
      if (row.kind === "anilist") await onDeleteHistory?.(row.item);
      else await onDeleteSourceHistory?.(row.item);
    } catch {
      // The parent callback reports failures.
    } finally {
      deletingKey = null;
    }
  }

  $effect(() => {
    if (!rangeOpen) return;
    const onPointer = (event: PointerEvent) => {
      const target = event.target as Node | null;
      if (rangeWrap && target && !rangeWrap.contains(target)) rangeOpen = false;
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") rangeOpen = false;
    };
    window.addEventListener("pointerdown", onPointer);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onPointer);
      window.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <PageHeader
    eyebrow="Activity"
    title="History"
    subtitle="AniList watched events and source progress, newest first, stored on this machine."
  >
    {#if merged.length > 0}
      <div class="history-toolbar">
        <p class="history-summary">{summary}</p>
        <div class="range-wrap" bind:this={rangeWrap}>
          <button
            class="btn-ghost toolbar-btn"
            onclick={() => (rangeOpen = !rangeOpen)}
            aria-haspopup="menu"
            aria-expanded={rangeOpen}
            aria-label="Time range, {rangeLabel}"
          >
            <Icon name="clock" size={13} />
            <span>{rangeLabel}</span>
            <Icon name="chevronRight" size={11} class="rotate-90" />
          </button>
          {#if rangeOpen}
            <div class="range-menu" role="menu" aria-label="Time range">
              {#each ranges as entry}
                <button
                  class="range-item"
                  class:is-active={range === entry.key}
                  role="menuitemradio"
                  aria-checked={range === entry.key}
                  onclick={() => selectRange(entry.key)}
                >
                  <span>{entry.label}</span>
                  {#if range === entry.key}
                    <Icon name="check" size={13} />
                  {/if}
                </button>
              {/each}
            </div>
          {/if}
        </div>
        {#if onClearHistory}
          <button
            class="btn-ghost toolbar-btn clear-action"
            onclick={openClear}
            disabled={clearing}
            aria-busy={clearing}
          >
            <Icon name="trash" size={13} />
            <span>Clear history</span>
          </button>
        {/if}
      </div>
    {/if}
  </PageHeader>

  {#if loading && merged.length === 0}
    <Skeleton kind="list" />
  {:else if error && merged.length === 0}
    <div class="surface p-5 max-w-xl">
      <div class="flex items-start gap-3">
        <span class="w-9 h-9 grid place-items-center rounded-lg shrink-0" style="background: var(--danger-wash); color: var(--danger);">
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
  {:else if merged.length === 0}
    <EmptyState
      icon="history"
      title="Nothing watched yet"
      body="Mark an episode watched from a series page or start a source episode and it lands here with the time it happened."
      actionLabel="Browse trending"
      onAction={onBrowse}
    />
  {:else if filtered.length === 0}
    <EmptyState
      compact
      icon="filter"
      title="Nothing in this range"
      body="No activity {emptyRangeLabel}. Try a wider time range or show everything again."
      actionLabel="Show all"
      onAction={() => selectRange("all")}
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
            {#each group.entries as row, position (rowKey(row))}
              <div
                class="history-row"
                style="border-top: {position > 0 ? '1px solid var(--line)' : 'none'};"
              >
                {#if row.kind === "anilist"}
                  <button
                    class="history-open"
                    onclick={() => onOpen(row.item.media)}
                    aria-label="Open {displayTitle(row.item.media)}, episode {row.item.episode}"
                  >
                    <div class="w-10 h-14 shrink-0 rounded-lg overflow-hidden" style="background: var(--void-700); border: 1px solid var(--line);">
                      <Artwork src={coverOf(row.item.media)} alt="" loading="lazy" />
                    </div>
                    <div class="min-w-0 flex-1">
                      <p class="text-sm font-semibold truncate">{displayTitle(row.item.media)}</p>
                      <p class="text-xs mt-0.5" style="color: var(--paper-450);">
                        Episode {row.item.episode}
                        {#if row.item.media.episodes} of {row.item.media.episodes}{/if}
                      </p>
                    </div>
                    <span class="chip chip-live text-[10px] px-2 py-0.5 hidden sm:inline-flex">
                      <Icon name="check" size={10} />
                      Watched
                    </span>
                    <div class="text-right shrink-0 min-w-[72px]">
                      <p class="text-[11px]" style="color: var(--paper-300); font-family: var(--font-mono);">
                        {timeAgo(row.at)}
                      </p>
                    </div>
                  </button>
                  {#if onDeleteHistory}
                    <button
                      class="row-trash"
                      onclick={() => void deleteRow(row)}
                      disabled={deletingKey !== null}
                      aria-label="Delete {displayTitle(row.item.media)} episode {row.item.episode} from history"
                      title="Delete from history"
                    >
                      <Icon name="trash" size={14} />
                    </button>
                  {/if}
                {:else}
                  <button
                    class="history-open"
                    onclick={() => onOpenSource(row.item)}
                    aria-label="Open {row.item.title}, episode {row.item.episodeNumber}"
                  >
                    <div class="w-10 h-14 shrink-0 rounded-lg overflow-hidden" style="background: var(--void-700); border: 1px solid var(--line);">
                      <Artwork src={row.item.cover} alt="" loading="lazy" />
                    </div>
                    <div class="min-w-0 flex-1">
                      <p class="text-sm font-semibold truncate">{row.item.title}</p>
                      <p class="text-xs mt-0.5 truncate" style="color: var(--paper-450);">
                        Episode {row.item.episodeNumber}
                        {#if row.item.episodeTitle} · {row.item.episodeTitle}{/if}
                        <span style="color: var(--paper-500);"> / {row.item.extensionName}</span>
                      </p>
                    </div>
                    {#if row.item.watched}
                      <span class="chip chip-live text-[10px] px-2 py-0.5 hidden sm:inline-flex">
                        <Icon name="check" size={10} />
                        Watched
                      </span>
                    {:else}
                      <span class="chip chip-volt text-[10px] px-2 py-0.5 hidden sm:inline-flex">
                        <Icon name="playCircle" size={10} />
                        In progress
                      </span>
                    {/if}
                    <div class="text-right shrink-0 min-w-[72px]">
                      <p class="text-[11px]" style="color: var(--paper-300); font-family: var(--font-mono);">
                        {timeAgo(row.at)}
                      </p>
                    </div>
                  </button>
                  {#if onDeleteSourceHistory}
                    <button
                      class="row-trash"
                      onclick={() => void deleteRow(row)}
                      disabled={deletingKey !== null}
                      aria-label="Delete {row.item.title} episode {row.item.episodeNumber} from history"
                      title="Delete from history"
                    >
                      <Icon name="trash" size={14} />
                    </button>
                  {/if}
                {/if}
              </div>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  {/if}
</div>

<Modal
  open={clearOpen}
  size="sm"
  overlayName="history-clear"
  label="Clear history"
  onClose={closeClear}
  headerClass="px-5 py-4"
  bodyClass="px-5 py-5"
  footerClass="px-5 py-3 flex items-center gap-2"
>
  {#snippet header()}
    <div class="flex items-center gap-3">
      <span
        class="w-9 h-9 grid place-items-center rounded-xl shrink-0"
        style="background: var(--danger-wash); border: 1px solid var(--danger-line); color: var(--danger);"
      >
        <Icon name="trash" size={17} />
      </span>
      <div class="min-w-0 flex-1">
        <h2 class="text-base font-semibold">Clear history</h2>
        <p class="text-[11px]" style="color: var(--text-4);">This cannot be undone.</p>
      </div>
      <button class="btn-ghost px-2.5 py-1.5 text-xs" onclick={closeClear} disabled={clearing} aria-label="Close">
        <Icon name="close" size={14} />
      </button>
    </div>
  {/snippet}

  <p class="text-sm leading-relaxed" style="color: var(--paper-300);">
    This removes every AniList and source activity entry and clears Continue Watching for both.
    Local resume positions are cleared. Your library, watched marks and AniList list stay.
  </p>

  {#snippet footer()}
    <button class="btn-ghost px-4 py-2 text-sm" onclick={closeClear} disabled={clearing} data-autofocus>
      Cancel
    </button>
    <button
      class="btn-ghost danger-action px-4 py-2 text-sm ml-auto"
      onclick={() => void clearHistory()}
      disabled={clearing || !onClearHistory}
      aria-busy={clearing}
    >
      <Icon name="trash" size={14} />
      {clearing ? "Clearing…" : "Clear history"}
    </button>
  {/snippet}
</Modal>

<style>
  .history-toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .history-summary {
    margin-right: 0.25rem;
    color: var(--paper-450);
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .toolbar-btn {
    height: 32px;
    padding: 0 0.625rem;
    font-size: 11px;
  }

  .toolbar-btn:hover {
    transform: none;
  }

  .clear-action:hover:not(:disabled) {
    background: var(--danger-wash);
    border-color: var(--danger-line);
    color: var(--danger);
  }

  .range-wrap {
    position: relative;
  }

  .range-menu {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    z-index: 40;
    min-width: 190px;
    padding: 6px;
    border-radius: 14px;
    background: var(--glass-strong);
    border: 1px solid var(--line-strong);
    box-shadow: var(--shadow-pop);
    backdrop-filter: blur(24px) saturate(150%);
    -webkit-backdrop-filter: blur(24px) saturate(150%);
    animation: rangeIn 120ms var(--ease-out) both;
  }

  @keyframes rangeIn {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .range-item {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 9px;
    color: var(--paper-300);
    font-size: 12px;
    font-weight: 550;
    text-align: left;
    transition: background 120ms ease-out, color 120ms ease-out;
  }

  .range-item:hover {
    background: var(--tint-1);
    color: var(--text-1);
  }

  .range-item.is-active {
    background: var(--accent-wash);
    color: var(--volt-200);
  }

  .history-row {
    display: flex;
    align-items: center;
    transition: background var(--dur-fast) var(--ease-out);
  }

  .history-row:hover,
  .history-row:focus-within {
    background: var(--tint-1);
  }

  .history-open {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    min-width: 0;
    padding: 10px 4px 10px 12px;
    text-align: left;
  }

  .history-open:focus-visible {
    outline: 2px solid var(--volt-400);
    outline-offset: -2px;
  }

  .row-trash {
    width: 32px;
    height: 32px;
    margin-right: 8px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    border-radius: 9px;
    color: var(--paper-450);
    opacity: 0.45;
    transition:
      opacity var(--dur-fast) var(--ease-out),
      background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .history-row:hover .row-trash,
  .history-row:focus-within .row-trash {
    opacity: 1;
  }

  .row-trash:hover:not(:disabled),
  .row-trash:focus-visible {
    background: var(--danger-wash);
    color: var(--danger);
    opacity: 1;
  }

  .row-trash:focus-visible {
    outline: 2px solid var(--danger);
    outline-offset: -2px;
  }

  .danger-action {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }

  .danger-action:hover:not(:disabled) {
    background: color-mix(in oklab, var(--danger) 86%, #000);
    border-color: var(--danger);
    color: #fff;
  }
</style>
