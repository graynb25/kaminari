<script lang="ts">
  import Artwork from "./Artwork.svelte";
  import Callout from "./Callout.svelte";
  import EmptyState from "./EmptyState.svelte";
  import Icon from "./Icon.svelte";
  import PageHeader from "./PageHeader.svelte";
  import { coverOf, displayTitle, metaOf } from "./format";
  import {
    countdownDetail,
    countdownLabel,
    formatScheduleDate,
    formatScheduleTime,
  } from "./countdown";
  import { groupByDay, laterEntries, nextDayWithEntries, rollingDays } from "./scheduleDays";
  import type {
    AiringScheduleEntry,
    LibraryItem,
    Media,
    SourceLibraryItem,
  } from "./types";

  type VisibleRow = {
    entry: AiringScheduleEntry;
    media: Media | null;
    source: SourceLibraryItem | null;
    title: string;
    cover: string | null;
    key: string;
  };

  let {
    items,
    library,
    sourceItems,
    loading = false,
    error = null,
    warning = null,
    fetchedAt = null,
    stale = false,
    online = true,
    onRefresh,
    onOpen,
    onOpenSource,
    onBrowse,
    onSearch,
  }: {
    items: AiringScheduleEntry[];
    library: LibraryItem[];
    sourceItems: SourceLibraryItem[];
    loading?: boolean;
    error?: string | null;
    warning?: string | null;
    fetchedAt?: number | null;
    stale?: boolean;
    online?: boolean;
    onRefresh: () => void;
    onOpen: (media: Media) => void;
    onOpenSource: (item: SourceLibraryItem) => void;
    onBrowse: () => void;
    onSearch: () => void;
  } = $props();

  let now = $state(Math.floor(Date.now() / 1000));
  let selectedKey = $state<string | null>(null);

  $effect(() => {
    now = Math.floor(Date.now() / 1000);
    const id = window.setInterval(() => {
      if (document.visibilityState === "visible") now = Math.floor(Date.now() / 1000);
    }, 1000);
    const onVis = () => {
      if (document.visibilityState === "visible") now = Math.floor(Date.now() / 1000);
    };
    document.addEventListener("visibilitychange", onVis);
    return () => {
      window.clearInterval(id);
      document.removeEventListener("visibilitychange", onVis);
    };
  });

  // Keep the schedule fresh while the view stays open. A hidden window
  // skips the call and waits for the next visible tick.
  $effect(() => {
    if (!online) return;
    const id = window.setInterval(() => {
      if (document.visibilityState === "visible") onRefresh();
    }, 30 * 60 * 1000);
    return () => window.clearInterval(id);
  });

  const libraryById = $derived(new Map(library.map((entry) => [entry.media.id, entry])));
  const sourceByAnilist = $derived.by(() => {
    const map = new Map<number, SourceLibraryItem>();
    for (const item of sourceItems) {
      if (item.anilistId != null && !map.has(item.anilistId)) map.set(item.anilistId, item);
    }
    return map;
  });

  const visibleRows = $derived.by(() => {
    const rows: VisibleRow[] = [];
    for (const entry of items) {
      const lib = libraryById.get(entry.anilistId);
      if (lib) {
        if (lib.status === "dropped") continue;
        rows.push({
          entry,
          media: lib.media,
          source: null,
          title: displayTitle(lib.media),
          cover: coverOf(lib.media),
          key: `${entry.anilistId}:${entry.airingAt}`,
        });
        continue;
      }
      const src = sourceByAnilist.get(entry.anilistId);
      if (src) {
        rows.push({
          entry,
          media: null,
          source: src,
          title: src.title,
          cover: src.cover ?? null,
          key: `${entry.anilistId}:${entry.airingAt}`,
        });
      }
    }
    rows.sort((a, b) => a.entry.airingAt - b.entry.airingAt);
    return rows;
  });

  const visibleEntries = $derived(visibleRows.map((row) => row.entry));
  const rowByKey = $derived(new Map(visibleRows.map((row) => [row.key, row])));
  const distinctCount = $derived(new Set(visibleRows.map((row) => row.entry.anilistId)).size);

  const days = $derived(rollingDays(now));
  const grouped = $derived(groupByDay(visibleEntries, days));
  const later = $derived(laterEntries(visibleEntries, days, now));

  $effect(() => {
    const keys = new Set(grouped.map((day) => day.key));
    if (!selectedKey || !keys.has(selectedKey)) {
      selectedKey =
        nextDayWithEntries(grouped, grouped[0]?.key) ?? grouped[0]?.key ?? null;
    }
  });

  const selectedDay = $derived(grouped.find((day) => day.key === selectedKey) ?? null);
  const selectedRows = $derived.by(() => {
    if (!selectedDay) return [];
    const out: VisibleRow[] = [];
    for (const entry of selectedDay.entries) {
      const row = rowByKey.get(`${entry.anilistId}:${entry.airingAt}`);
      if (row) out.push(row);
    }
    return out;
  });

  const nextUp = $derived.by(() => {
    const cutoff = now - 30 * 60;
    for (const row of visibleRows) {
      if (row.entry.airingAt >= cutoff) return row;
    }
    return null;
  });

  // AniList advances nextAiringEpisode after an episode airs. Ask once,
  // five minutes later, so the next episode replaces the one that just aired.
  $effect(() => {
    const airingAt = nextUp?.entry.airingAt;
    if (!airingAt) return;
    const delay = Math.max(0, (airingAt + 5 * 60) * 1000 - Date.now());
    const id = window.setTimeout(() => {
      if (document.visibilityState === "visible") onRefresh();
    }, delay);
    return () => window.clearTimeout(id);
  });

  const subtitle = $derived.by(() => {
    if (visibleRows.length === 0) return "Nothing airing this week.";
    const word = distinctCount === 1 ? "show" : "shows";
    if (!nextUp) return `${distinctCount} ${word} airing. Nothing upcoming right now.`;
    const label = countdownLabel(nextUp.entry.airingAt, now);
    if (label.tone === "live") return `${distinctCount} ${word} airing. Airing now.`;
    return `${distinctCount} ${word} airing. Next episode ${label.text}.`;
  });

  const updatedLabel = $derived(
    fetchedAt ? `Updated ${formatScheduleTime(fetchedAt)}${stale ? " (cached)" : ""}` : "Not updated yet",
  );
  const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone ?? "local";
  const hasLibrary = $derived(library.length > 0 || sourceItems.length > 0);

  function openRow(row: VisibleRow) {
    if (row.media) onOpen(row.media);
    else if (row.source) onOpenSource(row.source);
  }

  function moveDay(delta: number) {
    if (grouped.length === 0) return;
    const idx = grouped.findIndex((day) => day.key === selectedKey);
    const base = idx >= 0 ? idx : 0;
    const next = Math.min(grouped.length - 1, Math.max(0, base + delta));
    selectedKey = grouped[next].key;
    document.getElementById(`sched-day-${selectedKey}`)?.focus();
  }

  function onStripKey(event: KeyboardEvent) {
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      moveDay(-1);
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      moveDay(1);
    } else if (event.key === "Home") {
      event.preventDefault();
      if (grouped[0]) {
        selectedKey = grouped[0].key;
        document.getElementById(`sched-day-${selectedKey}`)?.focus();
      }
    } else if (event.key === "End") {
      event.preventDefault();
      const last = grouped[grouped.length - 1];
      if (last) {
        selectedKey = last.key;
        document.getElementById(`sched-day-${selectedKey}`)?.focus();
      }
    }
  }

  function jumpToNext() {
    const next = nextDayWithEntries(grouped, selectedKey);
    if (next) {
      selectedKey = next;
      document.getElementById(`sched-day-${next}`)?.focus();
    }
  }

  function laterTitle(row: VisibleRow): string {
    return row.media ? (metaOf(row.media) || `Episode ${row.entry.episode}`) : (row.source?.extensionName ?? `Episode ${row.entry.episode}`);
  }
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <PageHeader eyebrow="Estimated" title="Airing Schedule" subtitle={subtitle}>
    <span class="text-xs mono" style="color: var(--paper-400);">{updatedLabel}</span>
    <button class="btn-ghost px-3.5 py-2 text-xs" onclick={onRefresh} disabled={loading} aria-label="Refresh schedule">
      <Icon name="refresh" size={14} />
      {loading ? "Refreshing" : "Refresh"}
    </button>
  </PageHeader>

  {#if loading}
    <p class="sr-only" role="status" aria-live="polite">Refreshing schedule</p>
  {/if}

  {#if loading && items.length === 0}
    <div aria-label="Loading schedule">
      <div class="shimmer sched-next-skeleton"></div>
      <div class="sched-strip" aria-hidden="true">
        {#each Array(7) as _}
          <div class="shimmer sched-chip-skeleton"></div>
        {/each}
      </div>
      <div class="sched-list">
        {#each Array(4) as _}
          <div class="shimmer sched-row-skeleton"></div>
        {/each}
      </div>
    </div>
  {:else if !hasLibrary}
    <EmptyState
      icon="library"
      title="No airing shows yet"
      body="Save airing anime to your library and their next episodes will appear here."
      actionLabel="Browse"
      actionIcon="arrowRight"
      onAction={onBrowse}
      secondaryLabel="Search titles"
      onSecondary={onSearch}
    />
  {:else if !online && items.length === 0}
    <EmptyState
      icon="error"
      title="Schedule needs a connection"
      body="The schedule needs AniList once. Reconnect to load it."
      actionLabel="Retry"
      actionIcon="refresh"
      onAction={onRefresh}
    />
  {:else if error && items.length === 0}
    <EmptyState
      icon="error"
      title="Airing schedule did not load"
      body={error}
      actionLabel="Retry"
      actionIcon="refresh"
      onAction={onRefresh}
    />
  {:else if visibleRows.length === 0}
    <EmptyState
      icon="sparkles"
      title="Nothing airing right now"
      body="Nothing in your library is airing right now."
      actionLabel="Browse airing"
      actionIcon="arrowRight"
      onAction={onBrowse}
    />
  {:else}
    {#if warning}
      <Callout tone="warning" class="mb-4" actionLabel="Retry" actionIcon="refresh" onAction={onRefresh}>
        {warning}
      </Callout>
    {/if}

    {#if nextUp}
      {@const nextLabel = countdownLabel(nextUp.entry.airingAt, now)}
      <button class="sched-next surface text-left w-full" onclick={() => openRow(nextUp)} aria-label="{nextUp.title}, episode {nextUp.entry.episode}, {nextLabel.aria}">
        <span class="sched-next-poster">
          {#if nextUp.cover}
            <Artwork src={nextUp.cover} alt="" loading="eager" />
          {/if}
        </span>
        <span class="min-w-0 flex-1">
          <span class="block text-[11px] font-semibold uppercase tracking-[0.16em]" style="color: var(--volt-300);">Next up</span>
          <span class="block mt-1 text-lg font-semibold leading-tight truncate" style="font-family: var(--font-display);">{nextUp.title}</span>
          <span class="mt-2 flex flex-wrap items-center gap-2">
            <span class="sched-ep">EP {nextUp.entry.episode}</span>
            <span class="text-xs" style="color: var(--paper-300);">
              {formatScheduleDate(nextUp.entry.airingAt)} at {formatScheduleTime(nextUp.entry.airingAt)}
            </span>
          </span>
          <span class="sched-detail mono">{countdownDetail(nextUp.entry.airingAt, now)}</span>
        </span>
      </button>
    {/if}

    <div class="sched-strip" role="tablist" aria-label="Airing days" tabindex="0" onkeydown={onStripKey}>
      {#each grouped as day}
        {@const active = day.key === selectedKey}
        <button
          id="sched-day-{day.key}"
          role="tab"
          aria-selected={active}
          aria-controls="sched-panel"
          class="sched-day {active ? 'is-selected' : ''}"
          onclick={() => (selectedKey = day.key)}
        >
          <span class="sched-weekday">{day.weekday}</span>
          <span class="sched-monthday">{day.monthDay}</span>
          {#if day.entries.length > 0}
            <span class="sched-count" aria-label="{day.entries.length} airing">
              <span class="sched-dot" aria-hidden="true"></span>
              <span class="mono">{day.entries.length}</span>
            </span>
          {/if}
        </button>
      {/each}
    </div>

    <div role="tabpanel" id="sched-panel" aria-labelledby="sched-day-{selectedKey}" tabindex="0" class="sched-panel">
      {#if selectedRows.length === 0}
        <div class="surface px-4 py-6 text-center">
          <p class="text-sm font-semibold">No episodes on this day.</p>
          {#if visibleRows.length > 0}
            <button class="btn-ghost px-3.5 py-2 text-xs mt-3" onclick={jumpToNext}>
              Jump to next airing day
            </button>
          {/if}
        </div>
      {:else}
        <div class="sched-list">
          {#each selectedRows as row}
            {@const label = countdownLabel(row.entry.airingAt, now)}
            <button
              class="sched-row {label.tone === 'past' ? 'is-past' : ''}"
              onclick={() => openRow(row)}
              aria-label="{row.title}, episode {row.entry.episode}, {label.aria}"
            >
              <span class="sched-time mono">{formatScheduleTime(row.entry.airingAt)}</span>
              <span class="sched-poster">
                {#if row.cover}
                  <Artwork src={row.cover} alt="" loading="lazy" />
                {/if}
              </span>
              <span class="min-w-0 flex-1">
                <span class="block text-sm font-semibold leading-snug truncate">{row.title}</span>
                <span class="block text-[11px] mt-0.5 truncate" style="color: var(--paper-400);">
                  {#if row.media}
                    {metaOf(row.media) || `Episode ${row.entry.episode}`}
                  {:else}
                    {row.source?.extensionName ?? `Episode ${row.entry.episode}`}
                  {/if}
                </span>
              </span>
              <span class="sched-ep">EP {row.entry.episode}</span>
              <span class="sched-pill" data-tone={label.tone}>{label.text}</span>
              <span class="sched-dot-row" aria-hidden="true"></span>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    {#if later.length > 0}
      <section aria-label="Later" class="mt-8">
        <div class="flex items-center gap-3 mb-3">
          <h2 class="shelf-title">Later</h2>
          <span class="shelf-count">{later.length}</span>
        </div>
        {#if later.length > 5}
          <details class="sched-later">
            <summary class="btn-ghost px-3.5 py-2 text-xs w-fit">Show all {later.length} later episodes</summary>
            <div class="sched-list mt-3">
              {#each later as entry}
                {@const row = rowByKey.get(`${entry.anilistId}:${entry.airingAt}`)}
                {#if row}
                  <button class="sched-row" onclick={() => openRow(row)} aria-label="{row.title}, episode {row.entry.episode}, {formatScheduleDate(row.entry.airingAt)} at {formatScheduleTime(row.entry.airingAt)}">
                    <span class="sched-time mono">{formatScheduleDate(row.entry.airingAt)}</span>
                    <span class="sched-poster">
                      {#if row.cover}
                        <Artwork src={row.cover} alt="" loading="lazy" />
                      {/if}
                    </span>
                    <span class="min-w-0 flex-1">
                      <span class="block text-sm font-semibold leading-snug truncate">{row.title}</span>
                      <span class="block text-[11px] mt-0.5" style="color: var(--paper-400);">
                        {formatScheduleTime(row.entry.airingAt)} · {laterTitle(row)}
                      </span>
                    </span>
                    <span class="sched-ep">EP {row.entry.episode}</span>
                  </button>
                {/if}
              {/each}
            </div>
          </details>
        {:else}
          <div class="sched-list">
            {#each later as entry}
              {@const row = rowByKey.get(`${entry.anilistId}:${entry.airingAt}`)}
              {#if row}
                <button class="sched-row" onclick={() => openRow(row)} aria-label="{row.title}, episode {row.entry.episode}, {formatScheduleDate(row.entry.airingAt)} at {formatScheduleTime(row.entry.airingAt)}">
                  <span class="sched-time mono">{formatScheduleDate(row.entry.airingAt)}</span>
                  <span class="sched-poster">
                    {#if row.cover}
                      <Artwork src={row.cover} alt="" loading="lazy" />
                    {/if}
                  </span>
                  <span class="min-w-0 flex-1">
                    <span class="block text-sm font-semibold leading-snug truncate">{row.title}</span>
                    <span class="block text-[11px] mt-0.5" style="color: var(--paper-400);">
                      {formatScheduleTime(row.entry.airingAt)} · {laterTitle(row)}
                    </span>
                  </span>
                  <span class="sched-ep">EP {row.entry.episode}</span>
                </button>
              {/if}
            {/each}
          </div>
        {/if}
      </section>
    {/if}
  {/if}

  <p class="mt-8 text-xs leading-relaxed" style="color: var(--paper-500);">
    Times come from AniList and are estimates. Shown in your local timezone ({timeZone}).
  </p>
</div>

<style>
  .sched-next {
    display: flex;
    gap: 16px;
    align-items: center;
    padding: 16px;
    margin-bottom: 20px;
  }

  .sched-next:focus-visible,
  .sched-day:focus-visible,
  .sched-row:focus-visible {
    outline: 2px solid var(--volt-400);
    outline-offset: 2px;
  }

  .sched-next-poster {
    width: 96px;
    height: 144px;
    flex-shrink: 0;
    border-radius: var(--radius-md);
    overflow: hidden;
    background: var(--void-700);
  }

  .sched-detail {
    display: block;
    margin-top: 8px;
    font-size: 28px;
    font-weight: 700;
    color: var(--text-1);
    font-family: var(--font-display);
  }

  .sched-next-skeleton {
    height: 176px;
    border-radius: var(--radius-lg);
    margin-bottom: 20px;
  }

  .sched-strip {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 8px;
    margin: 4px 0 16px;
  }

  .sched-day {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 10px 6px;
    border-radius: var(--radius-md);
    background: var(--control-bg);
    border: 1px solid var(--line);
    color: var(--text-2);
    min-height: 72px;
  }

  .sched-day.is-selected {
    background: linear-gradient(180deg, var(--volt-400), var(--volt-500));
    border-color: rgb(255 255 255 / 0.18);
    color: #ffffff;
    transform: scale(1.05);
    box-shadow: 0 8px 22px var(--accent-shadow);
  }

  .sched-weekday {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .sched-monthday {
    font-size: 13px;
    font-weight: 600;
  }

  .sched-day.is-selected .sched-monthday {
    font-size: 15px;
  }

  .sched-count {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
  }

  .sched-dot {
    width: 6px;
    height: 6px;
    border-radius: 999px;
    background: var(--volt-400);
  }

  .sched-day.is-selected .sched-dot {
    background: #ffffff;
  }

  .sched-chip-skeleton {
    height: 72px;
    border-radius: var(--radius-md);
  }

  .sched-panel {
    outline: none;
  }

  .sched-list {
    position: relative;
    margin-left: 6px;
    border-left: 2px solid var(--line-strong);
    padding-left: 14px;
    display: grid;
    gap: 8px;
  }

  .sched-row {
    position: relative;
    display: grid;
    grid-template-columns: 64px 48px minmax(0, 1fr) auto auto;
    gap: 12px;
    align-items: center;
    width: 100%;
    text-align: left;
    padding: 8px 12px;
    border-radius: var(--radius-md);
    background: var(--control-bg);
    border: 1px solid var(--line);
    color: var(--text-1);
  }

  .sched-row:hover {
    border-color: var(--line-strong);
  }

  .sched-row.is-past {
    opacity: 0.62;
  }

  .sched-dot-row {
    position: absolute;
    left: -20px;
    top: 50%;
    width: 8px;
    height: 8px;
    margin-top: -4px;
    border-radius: 999px;
    background: var(--volt-400);
    box-shadow: 0 0 10px var(--volt-400);
  }

  .sched-row.is-past .sched-dot-row {
    background: var(--paper-500);
    box-shadow: none;
  }

  .sched-time {
    font-size: 12px;
    color: var(--text-2);
  }

  .sched-poster {
    width: 48px;
    height: 64px;
    border-radius: 8px;
    overflow: hidden;
    background: var(--void-700);
  }

  .sched-ep {
    display: inline-flex;
    align-items: center;
    height: 22px;
    padding: 0 8px;
    border-radius: 999px;
    background: var(--accent-wash);
    border: 1px solid var(--line-volt);
    color: var(--volt-200);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    white-space: nowrap;
  }

  .sched-pill {
    display: inline-flex;
    align-items: center;
    height: 24px;
    padding: 0 10px;
    border-radius: 999px;
    border: 1px solid var(--line);
    background: var(--tint-1);
    color: var(--text-2);
    font-size: 11px;
    font-weight: 650;
    white-space: nowrap;
  }

  .sched-pill[data-tone="future"] {
    background: var(--accent-wash);
    border-color: var(--line-volt);
    color: var(--volt-200);
  }

  .sched-pill[data-tone="soon"] {
    background: var(--warning-wash);
    border-color: var(--warning-line);
    color: var(--warning);
  }

  .sched-pill[data-tone="live"] {
    background: var(--success-wash);
    border-color: var(--success-line);
    color: var(--success);
  }

  .sched-pill[data-tone="past"] {
    color: var(--paper-500);
  }

  .sched-row-skeleton {
    height: 80px;
    border-radius: var(--radius-md);
  }

  .sched-later summary {
    cursor: pointer;
    list-style: none;
  }
</style>
