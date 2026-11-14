<script lang="ts">
  import { onMount } from "svelte";
  import { dismissOnboarding, shouldShowOnboarding } from "./lib/onboarding";
  import { incognitoEnabled, setIncognitoEnabled } from "./lib/incognito";
  import Callout from "./lib/Callout.svelte";
  import ContinueCard from "./lib/ContinueCard.svelte";
  import DetailPage from "./lib/DetailPage.svelte";
  import EmptyState from "./lib/EmptyState.svelte";
  import FatalState from "./lib/FatalState.svelte";
  import AppFooter from "./lib/AppFooter.svelte";
  import GettingStarted from "./lib/GettingStarted.svelte";
  import HeroCarousel from "./lib/HeroCarousel.svelte";
  import HistoryView from "./lib/HistoryView.svelte";
  import Icon from "./lib/Icon.svelte";
  import LibraryView from "./lib/LibraryView.svelte";
  import LicensesDialog from "./lib/LicensesDialog.svelte";
  import ScheduleView from "./lib/ScheduleView.svelte";
  import NavRail from "./lib/NavRail.svelte";
  import PosterCard from "./lib/PosterCard.svelte";
  import Rail from "./lib/Rail.svelte";
  import SearchPalette from "./lib/SearchPalette.svelte";
  import SettingsView from "./lib/SettingsView.svelte";
  import Shelf from "./lib/Shelf.svelte";
  import ShortcutsDialog from "./lib/ShortcutsDialog.svelte";
  import Skeleton from "./lib/Skeleton.svelte";
  import SourceContinueCard from "./lib/SourceContinueCard.svelte";
  import SourceBrowser from "./lib/SourceBrowser.svelte";
  import SourceMatchDialog from "./lib/SourceMatchDialog.svelte";
  import SourcesView from "./lib/SourcesView.svelte";
  import TitleBar from "./lib/TitleBar.svelte";
  import Toasts from "./lib/Toasts.svelte";
  import {
    addToLibrary,
    airingSchedule,
    applyHistoryRetention,
    bootSnapshot,
    continueWatching,
    detail as fetchDetail,
    historyClear,
    historyList,
    historyRemove,
    listLibrary,
    removeFromLibrary,
    setIncognitoMode,
    setLibraryStatus,
    sourceLibraryList,
    sourceContinue,
    sourceHistory,
    sourceHistoryRemove,
    shelf,
    watchedList,
    watchedMarkAll,
    watchedSet,
    type ShelfKind,
  } from "./lib/api";
  import { displayTitle, isRateLimitMessage } from "./lib/format";
  import { mergeContinueRows } from "./lib/continueRows";
  import {
    historyRetentionDays,
    readHistoryRetention,
  } from "./lib/historyRetention";
  import { onAniListSyncConflict } from "./lib/anilistSync";
  import type {
    AiringScheduleEntry,
    Boot,
    ContinueItem,
    HistoryItem,
    JsSearchItem,
    LibraryItem,
    Media,
    RailState,
    SourceExtensionRef,
    SourceContinueItem,
    SourceHistoryItem,
    SourceLibraryItem,
    Toast,
    ToastAction,
    ToastTone,
    View,
  } from "./lib/types";

  type SourceBrowserState = {
    extension: SourceExtensionRef;
    initialShow?: JsSearchItem;
    resumeEpisodeId?: string;
    anilistMedia?: Media;
    anilistId?: number | null;
  };


  let title = $state("Kaminari");
  let boot = $state<Boot | null>(null);
  let bootError = $state<string | null>(null);
  let library = $state<LibraryItem[]>([]);
  let libraryLoading = $state(true);
  let libraryError = $state<string | null>(null);
  let rails = $state<Record<string, RailState>>({
    trending: { items: [], loading: true, error: null },
    trendingMore: { items: [], loading: true, error: null },
    airing: { items: [], loading: true, error: null },
    top: { items: [], loading: true, error: null },
    popular: { items: [], loading: true, error: null },
    upcoming: { items: [], loading: true, error: null },
  });
  let view = $state<View>("browse");
  let detail = $state<Media | null>(null);
  let detailError = $state<string | null>(null);
  let sourceMatchMedia = $state<Media | null>(null);
  let watchedDetail = $state<number[]>([]);
  let continueItems = $state<ContinueItem[]>([]);
  let continueError = $state<string | null>(null);
  let historyItems = $state<HistoryItem[]>([]);
  let historyLoading = $state(true);
  let historyError = $state<string | null>(null);
  let sourceLibraryItems = $state<SourceLibraryItem[]>([]);
  let sourceContinueItems = $state<SourceContinueItem[]>([]);
  let sourceHistoryItems = $state<SourceHistoryItem[]>([]);
  let sourceBrowser = $state<SourceBrowserState | null>(null);
  let schedule = $state<AiringScheduleEntry[]>([]);
  let scheduleLoading = $state(false);
  let scheduleError = $state<string | null>(null);
  let scheduleWarning = $state<string | null>(null);
  let scheduleFetchedAt = $state<number | null>(null);
  let scheduleStale = $state(false);
  let onboardingOpen = $state(false);
  let incognito = $state(false);
  let paletteOpen = $state(false);
  let shortcutsOpen = $state(false);
  let licensesOpen = $state(false);
  let toasts = $state<Toast[]>([]);
  let toastSeq = $state(0);
  let mainEl = $state<HTMLElement | null>(null);
  let online = $state(typeof navigator !== "undefined" ? navigator.onLine : true);
  let detailRequest = 0;
  const viewNames: Record<View, string> = {
    browse: "Browse",
    schedule: "Schedule",
    library: "Library",
    history: "History",
    sources: "Sources",
    settings: "Settings",
  };
  let announcedLocation = "browse";
  let routeAnnouncement = $state("");

  const trendingRail = $derived(rails.trending);

  const continueRows = $derived(mergeContinueRows(continueItems, sourceContinueItems));
  const libraryIds = $derived(new Set(library.map((entry) => entry.media.id)));
  const detailEntry = $derived.by(() => {
    if (!detail) return undefined;
    const id = detail.id;
    return library.find((entry) => entry.media.id === id);
  });

  function notify(text: string, tone: ToastTone = "info", action?: ToastAction) {
    toastSeq += 1;
    const id = toastSeq;
    toasts = [...toasts.slice(-2), { id, text, tone, action }];
    window.setTimeout(() => dismissToast(id), 4200);
  }

  function dismissToast(id: number) {
    toasts = toasts.filter((toast) => toast.id !== id);
  }

  function isInLibrary(id: number): boolean {
    return library.some((entry) => entry.media.id === id);
  }

  function openPalette() {
    paletteOpen = true;
  }

  function closePalette() {
    paletteOpen = false;
  }

  function goView(next: View) {
    detailRequest += 1;
    detail = null;
    detailError = null;
    watchedDetail = [];
    view = next;
    if (next === "history") {
      void refreshHistory();
      void refreshSourceHistory();
    }
    if (next === "schedule") {
      void refreshSchedule(false);
    }
  }

  async function refreshSchedule(force = false) {
    if (scheduleLoading) return;
    scheduleLoading = true;
    try {
      let result = await airingSchedule(force);
      schedule = result.items;
      scheduleFetchedAt = result.fetchedAt;
      scheduleStale = result.stale;
      scheduleWarning = result.warning;
      scheduleError = null;
      if (!force && result.stale && !result.warning && online) {
        result = await airingSchedule(true);
        schedule = result.items;
        scheduleFetchedAt = result.fetchedAt;
        scheduleStale = result.stale;
        scheduleWarning = result.warning;
        scheduleError = null;
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      if (schedule.length > 0) {
        scheduleWarning = message;
      } else {
        scheduleError = message;
      }
    } finally {
      scheduleLoading = false;
    }
  }

  async function refreshContinue() {
    try {
      continueItems = await continueWatching();
      continueError = null;
    } catch (e) {
      continueItems = [];
      continueError = e instanceof Error ? e.message : String(e);
    }
  }

  async function refreshHistory() {
    historyLoading = true;
    try {
      historyItems = await historyList(100);
      historyError = null;
    } catch (e) {
      historyItems = [];
      historyError = e instanceof Error ? e.message : String(e);
    } finally {
      historyLoading = false;
    }
  }

  async function refreshSourceLibrary() {
    try {
      sourceLibraryItems = await sourceLibraryList();
    } catch {
      sourceLibraryItems = [];
    }
  }

  async function refreshSourceContinue() {
    try {
      sourceContinueItems = await sourceContinue(20);
    } catch {
      sourceContinueItems = [];
    }
  }

  async function refreshSourceHistory() {
    try {
      sourceHistoryItems = await sourceHistory(100);
    } catch {
      sourceHistoryItems = [];
    }
  }

  async function clearHistory() {
    try {
      await historyClear();
      await Promise.all([
        refreshHistory(),
        refreshSourceHistory(),
        refreshContinue(),
        refreshSourceContinue(),
        refreshSourceLibrary(),
      ]);
      notify("History and Continue Watching cleared. Library and watched marks stay.", "success");
    } catch (e) {
      notify(`History could not be cleared: ${errorText(e)}`, "error");
    }
  }

  async function removeHistoryEntry(item: HistoryItem) {
    try {
      await historyRemove(item.media.id, item.episode);
      historyItems = historyItems.filter(
        (entry) => !(entry.media.id === item.media.id && entry.episode === item.episode),
      );
      notify(`Removed ${displayTitle(item.media)} episode ${item.episode} from history.`, "success");
    } catch (e) {
      notify(`History entry could not be removed: ${errorText(e)}`, "error");
    }
  }

  async function removeSourceHistoryEntry(item: SourceHistoryItem) {
    try {
      await sourceHistoryRemove(item.extensionId, item.showId, item.episodeId);
      sourceHistoryItems = sourceHistoryItems.filter(
        (entry) =>
          !(
            entry.extensionId === item.extensionId &&
            entry.showId === item.showId &&
            entry.episodeId === item.episodeId
          ),
      );
      notify(`Removed ${item.title} episode ${item.episodeNumber} from history.`, "success");
    } catch (e) {
      notify(`History entry could not be removed: ${errorText(e)}`, "error");
    }
  }

  async function refreshSourceActivity() {
    await Promise.all([
      refreshSourceLibrary(),
      refreshSourceContinue(),
      refreshSourceHistory(),
    ]);
  }

  async function refreshLibrary() {
    libraryLoading = true;
    try {
      library = await listLibrary();
      libraryError = null;
    } catch (e) {
      libraryError = e instanceof Error ? e.message : String(e);
    } finally {
      libraryLoading = false;
    }
  }

  // One rail owns one request. A failure stops at that rail, never the page.
  async function loadRail(key: string, kind: ShelfKind, page = 1) {
    rails[key] = { items: [], loading: true, error: null };
    try {
      rails[key] = { items: await shelf(kind, page), loading: false, error: null };
    } catch (e) {
      rails[key] = { items: [], loading: false, error: e instanceof Error ? e.message : String(e) };
    }
  }

  // Trending page 1 feeds the hero and the Top 10 rail. Trending page 2 feeds
  // the Trending Now shelf, so the same ten posters never repeat in a row.
  function loadBrowse() {
    return Promise.all([
      loadRail("trending", "trending", 1),
      loadRail("trendingMore", "trending", 2),
      loadRail("airing", "airing", 1),
      loadRail("top", "top", 1),
      loadRail("popular", "popular", 1),
      loadRail("upcoming", "upcoming", 1),
    ]);
  }

  async function loadBoot() {
    try {
      boot = await bootSnapshot();
      title = boot.title;
      bootError = null;
    } catch (e) {
      bootError = e instanceof Error ? e.message : String(e);
    }
  }

  function retryBoot() {
    void loadBoot();
  }

  function retryDetail() {
    if (detail) void openDetail(detail);
  }

  async function openDetail(item: Media) {
    const request = ++detailRequest;
    detailError = null;
    detail = item;
    mainEl?.scrollTo({ top: 0, behavior: "auto" });
    try {
      const full = await fetchDetail(item.id);
      if (request === detailRequest) detail = full;
    } catch (e) {
      if (request === detailRequest) {
        detailError = e instanceof Error ? e.message : String(e);
      }
    }
    try {
      const seen = await watchedList(item.id);
      if (request === detailRequest) watchedDetail = seen;
    } catch {
      if (request === detailRequest) watchedDetail = [];
    }
  }

  function openPlay(item: Media) {
    sourceMatchMedia = item;
  }

  function openSourceShow(item: {
    extensionId: string;
    extensionName: string;
    showId: string;
    title: string;
    cover?: string | null;
    year?: number | null;
    anilistId?: number | null;
    resumeEpisodeId?: string;
  }) {
    sourceBrowser = {
      extension: { id: item.extensionId, name: item.extensionName },
      initialShow: {
        id: item.showId,
        title: item.title,
        cover: item.cover ?? null,
        year: item.year ?? null,
      },
      resumeEpisodeId: item.resumeEpisodeId,
      anilistId: item.anilistId ?? null,
    };
  }

  function openSourceContinue(item: SourceContinueItem) {
    openSourceShow({
      extensionId: item.extensionId,
      extensionName: item.extensionName,
      showId: item.showId,
      title: item.title,
      cover: item.cover,
      anilistId: item.anilistId,
      resumeEpisodeId: item.episodeId,
    });
  }

  function resumeSource(item: SourceLibraryItem) {
    openSourceShow({
      extensionId: item.extensionId,
      extensionName: item.extensionName,
      showId: item.showId,
      title: item.title,
      cover: item.cover,
      year: item.year,
      anilistId: item.anilistId,
      resumeEpisodeId: item.lastEpisodeId ?? undefined,
    });
  }

  function openExtension(extension: { id: string; name: string }) {
    sourceBrowser = { extension: { id: extension.id, name: extension.name } };
  }

  async function toggleLibrary(item: Media) {
    const name = displayTitle(item);
    try {
      if (isInLibrary(item.id)) {
        await removeFromLibrary(item.id);
        notify(`Removed ${name} from your library.`, "success");
      } else {
        await addToLibrary(item, item.status === "NOT_YET_RELEASED" ? "planning" : "watching");
        notify(`Saved ${name} to your library.`, "success");
      }
      await refreshLibrary();
      await refreshContinue();
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Library change failed: ${message}`, "error", {
        label: "Retry",
        run: () => void toggleLibrary(item),
      });
    }
  }

  async function toggleWatched(episode: number) {
    if (!detail) return;
    if (incognito) {
      notify("Incognito mode is on. Turn it off in Settings to save progress.", "info");
      return;
    }
    const seen = watchedDetail.includes(episode);
    const previous = watchedDetail;
    // Optimistic first, then roll back if SQLite refuses the write.
    watchedDetail = seen
      ? previous.filter((value) => value !== episode)
      : [...previous, episode].sort((a, b) => a - b);
    try {
      watchedDetail = await watchedSet(detail, episode, !seen);
      await Promise.all([refreshLibrary(), refreshContinue(), refreshHistory()]);
    } catch (e) {
      watchedDetail = previous;
      const message = e instanceof Error ? e.message : String(e);
      notify(`Episode update failed: ${message}`, "error", {
        label: "Retry",
        run: () => void toggleWatched(episode),
      });
    }
  }

  async function markAllWatched(seen: boolean) {
    if (!detail) return;
    if (incognito) {
      notify("Incognito mode is on. Turn it off in Settings to save progress.", "info");
      return;
    }
    const through = detail.episodes ?? 0;
    if (through <= 0) return;
    try {
      watchedDetail = await watchedMarkAll(detail, through, seen);
      await Promise.all([refreshLibrary(), refreshContinue(), refreshHistory()]);
      notify(
        seen ? `Marked all ${through} episodes watched.` : "Cleared watched episodes.",
        "success",
      );
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Episode update failed: ${message}`, "error", {
        label: "Retry",
        run: () => void markAllWatched(seen),
      });
    }
  }

  async function setStatus(item: Media, status: string) {
    try {
      await setLibraryStatus(item.id, status);
      await refreshLibrary();
      notify(`${displayTitle(item)} set to ${status}.`, "success");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Status update failed: ${message}`, "error", {
        label: "Retry",
        run: () => void setStatus(item, status),
      });
    }
  }

  function closeDetail() {
    detailRequest += 1;
    detail = null;
    detailError = null;
  }

  function closeOnboarding() {
    onboardingOpen = false;
    dismissOnboarding();
  }

  function onboardingSources() {
    closeOnboarding();
    goView("sources");
  }

  function onboardingBrowse() {
    closeOnboarding();
    goView("browse");
  }

  function toggleIncognito(enabled: boolean) {
    incognito = enabled;
    setIncognitoEnabled(enabled);
    void setIncognitoMode(enabled).catch(() => {
      // The local flag still controls playback; the backend starts the next
      // sync tick once the command is available again.
    });
    notify(
      enabled
        ? "Incognito mode on. Playback and new searches will not be saved, and background AniList sync is paused."
        : "Incognito mode off. Playback and searches will be saved again.",
      "info",
    );
  }

  function errorText(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function typingTarget(event: KeyboardEvent): boolean {
    const target = event.target as HTMLElement | null;
    if (!target) return false;
    const tag = target.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.isContentEditable;
  }

  onMount(() => {
    // Start keyboard focus in the content, not on the window controls.
    mainEl?.focus();
    void (async () => {
      await loadBoot();
      const retentionDays = historyRetentionDays(readHistoryRetention());
      await applyHistoryRetention(retentionDays).catch(() => {
        // The window still applies on the next launch or when Settings changes it.
      });
      onboardingOpen = shouldShowOnboarding();
      incognito = incognitoEnabled();
      void setIncognitoMode(incognito).catch(() => {
        // A missing backend flag only affects the automatic sync tick.
      });
      await refreshLibrary();
      await refreshContinue();
      await refreshHistory();
      await refreshSourceActivity();
      await loadBrowse();
    })();
  });

  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void (async () => {
      try {
        const stop = await onAniListSyncConflict((result) => {
          if (disposed) return;
          notify(
            result.conflicts === 1
              ? "AniList had a newer change for 1 title. The AniList value was kept."
              : `AniList had newer changes for ${result.conflicts} titles. The AniList values were kept.`,
            "info",
          );
        });
        if (disposed) stop();
        else unlisten = stop;
      } catch {
        // Settings still reports conflicts when the event listener is unavailable.
      }
    })();
    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  // Keep the content at the top when the route or the open title changes.
  $effect(() => {
    view;
    detail?.id;
    if (mainEl) mainEl.scrollTop = 0;
  });

  // Announce route and detail changes for keyboard and screen reader users
  // without moving focus away from the control they just used.
  $effect(() => {
    const location = detail ? `detail-${detail.id}` : view;
    if (location === announcedLocation) return;
    announcedLocation = location;
    routeAnnouncement = detail ? `Details: ${displayTitle(detail)}` : `${viewNames[view]} view`;
  });

  // Online and offline banner.
  $effect(() => {
    const show = () => {
      online = true;
    };
    const hide = () => {
      online = false;
    };
    window.addEventListener("online", show);
    window.addEventListener("offline", hide);
    return () => {
      window.removeEventListener("online", show);
      window.removeEventListener("offline", hide);
    };
  });

  // Global keys. Palette input handles its own list keys.
  $effect(() => {
    const onKey = (event: KeyboardEvent) => {
      const overlayOpen = Boolean(document.body.dataset.overlay);
      if (document.body.dataset.playerOpen === "true" || overlayOpen) return;
      const mod = event.ctrlKey || event.metaKey;
      if (mod && event.key.toLowerCase() === "k") {
        event.preventDefault();
        if (paletteOpen) closePalette();
        else openPalette();
        return;
      }
      if (typingTarget(event) || paletteOpen) return;
      if (event.key === "?") {
        event.preventDefault();
        shortcutsOpen = !shortcutsOpen;
      } else if (event.key === "/") {
        event.preventDefault();
        openPalette();
      } else if (event.key === "Escape") {
        if (detail) closeDetail();
      } else if (event.key >= "1" && event.key <= "6") {
        const next: View[] = ["browse", "schedule", "library", "history", "sources", "settings"];
        const pick = next[Number(event.key) - 1];
        if (pick) goView(pick);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class="app-shell h-screen flex flex-col">
  <div class="app-atmosphere" aria-hidden="true"></div>
  <div class="app-grain" aria-hidden="true"></div>

  <a href="#content" class="skip-link">Skip to content</a>
  <p class="sr-only" role="status" aria-live="polite">{routeAnnouncement}</p>
  <TitleBar title={title} online={online} onOpenPalette={openPalette} />

  <div class="relative z-10 flex flex-1 min-h-0">
    <NavRail
      view={view}
      onNav={goView}
      onOpenPalette={openPalette}
      onOpenShortcuts={() => (shortcutsOpen = true)}
    />

    <!-- The skip link focuses this region without adding it to the tab order. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <main id="content" bind:this={mainEl} class="ambient flex-1 overflow-y-auto min-w-0" tabindex={-1}>
      <div class="min-h-full flex flex-col">
      {#if !online}
        <Callout tone="warning" icon="wifiOff" class="mx-4 mt-3 md:mx-6">
          You are offline. Showing cached titles. Search and detail refresh will fail until you reconnect.
        </Callout>
      {/if}

      {#if bootError && !boot}
        <div class="flex-1">
          <FatalState
            code="BOOT_SNAPSHOT_FAILED"
            title="Kaminari could not start"
            message="The app shell loaded, but the boot snapshot failed. Nothing was written or deleted. Retry the snapshot, then browse again."
            detail={bootError}
            onRetry={retryBoot}
          />
        </div>
      {:else}
        <svelte:boundary>
          {#key detail ? `detail-${detail.id}` : view}
            <div class="page-swap flex-1">
          {#if detail}
            {#if detailError}
              <div
                class="mx-6 md:mx-8 mt-6 px-4 py-3 rounded-xl text-sm flex flex-col sm:flex-row sm:items-center gap-2"
                style="background: var(--warning-wash); border: 1px solid var(--warning-line); color: var(--paper-300);"
                role="alert"
              >
                <Icon name="alert" size={15} />
                <span class="flex-1">Full details did not load ({detailError}). Showing the cached card.</span>
                <button class="btn-primary px-3 py-1.5 text-xs" onclick={retryDetail}>Retry</button>
              </div>
            {/if}
            <DetailPage
              media={detail}
              inLibrary={isInLibrary(detail.id)}
              libraryIds={libraryIds}
              status={detailEntry?.status ?? null}
              watched={watchedDetail}
              onBack={closeDetail}
              onToggleLibrary={toggleLibrary}
              onSetStatus={setStatus}
              onPlay={openPlay}
              onToggleWatched={(episode) => void toggleWatched(episode)}
              onMarkAll={(seen) => void markAllWatched(seen)}
              onOpenRecommendation={(item) => void openDetail(item)}
            />
          {:else if view === "browse"}
            {#if trendingRail.loading}
              <div class="space-y-6 pb-8">
                <Skeleton kind="hero" />
                <div class="px-6 md:px-8 space-y-8 max-w-[1440px] mx-auto">
                  <Skeleton kind="row" />
                  <Skeleton kind="row" />
                </div>
              </div>
            {:else if trendingRail.error}
              <div class="p-6 md:p-8 max-w-[1440px] mx-auto">
                <EmptyState
                  compact
                  icon="error"
                  title="AniList did not answer"
                  body={isRateLimitMessage(trendingRail.error)
                    ? "This is the public rate limit. It clears in under a minute. Other shelves load on their own."
                    : `${trendingRail.error}. Check your connection, then retry.`}
                  actionLabel="Retry"
                  actionIcon="refresh"
                  onAction={() => void loadRail("trending", "trending", 1)}
                />
              </div>
            {:else if trendingRail.items.length === 0}
              <div class="p-6 md:p-8 max-w-[1440px] mx-auto">
                <EmptyState
                  compact
                  icon="search"
                  title="Nothing trending right now"
                  body="AniList returned an empty shelf. Try again in a moment."
                  actionLabel="Reload"
                  actionIcon="refresh"
                  onAction={() => void loadRail("trending", "trending", 1)}
                />
              </div>
            {:else}
              <HeroCarousel
                items={trendingRail.items}
                isInLibrary={isInLibrary}
                onOpen={(item) => void openDetail(item)}
                onToggleLibrary={toggleLibrary}
                onPlay={openPlay}
              />
              <div class="px-6 md:px-8 py-6 space-y-8 max-w-[1440px] mx-auto">
                {#if library.length === 0 && continueRows.length === 0}
                  <div class="start-callout">
                    <span class="w-11 h-11 grid place-items-center rounded-xl shrink-0" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
                      <Icon name="sparkles" size={20} />
                    </span>
                    <div class="min-w-0 flex-1">
                      <p class="text-sm font-semibold">Build your collection</p>
                      <p class="text-xs mt-1 leading-relaxed" style="color: var(--paper-400);">
                        Save a poster and it follows you across Continue Watching, Library and History.
                        Search is the fastest way in.
                      </p>
                    </div>
                    <button class="btn-primary px-4 py-2 text-xs shrink-0" onclick={openPalette}>
                      <Icon name="search" size={13} />
                      Search titles
                      <kbd class="kbd">Ctrl K</kbd>
                    </button>
                  </div>
                {:else}
                  <section aria-label="Continue watching">
                    <Shelf
                      title="Continue Watching"
                      count={continueRows.length}
                      onSeeAll={() => goView("library")}
                    >
                      {#if continueError}
                        <div class="w-full surface px-4 py-4 flex items-center gap-3 text-sm" style="color: var(--paper-300);" role="alert">
                          <span class="w-8 h-8 grid place-items-center rounded-lg shrink-0" style="background: var(--danger-wash); color: var(--danger);">
                            <Icon name="alert" size={16} />
                          </span>
                          <span class="flex-1">Progress did not load: {continueError}</span>
                          <button class="btn-ghost px-3 py-1.5 text-[11px] shrink-0" onclick={() => void refreshContinue()}>Retry</button>
                        </div>
                      {:else if continueRows.length === 0}
                        <div class="w-full surface px-4 py-4 flex items-center gap-3 text-sm" style="color: var(--paper-300);">
                          <span class="w-8 h-8 grid place-items-center rounded-lg shrink-0" style="background: var(--accent-wash); color: var(--volt-300);">
                            <Icon name="playCircle" size={16} />
                          </span>
                          <span class="flex-1">Nothing in progress yet. Start a source episode and this row fills itself.</span>
                          <button class="btn-ghost px-3 py-1.5 text-[11px] shrink-0" onclick={() => goView("library")}>Open library</button>
                        </div>
                      {:else}
                        {#each continueRows as row (row.kind === "anilist" ? `a:${row.item.media.id}` : row.kind === "linked" ? `l:${row.anilist.media.id}` : `s:${row.item.extensionId}:${row.item.showId}`)}
                          <div class="w-[300px] md:w-[320px] shrink-0">
                            {#if row.kind === "anilist"}
                              <ContinueCard item={row.item} onOpen={(item) => void openDetail(item)} />
                            {:else if row.kind === "linked"}
                              <ContinueCard
                                item={row.anilist}
                                episode={row.source.episodeNumber}
                                tag={row.suggested
                                  ? "TITLE MATCH"
                                  : row.source.watched
                                    ? "REWATCH"
                                    : "RESUME"}
                                hint={row.suggested
                                  ? `Title match from ${row.source.extensionName}. Open the source to resume.`
                                  : `Resumes ${row.source.extensionName}.`}
                                onOpen={() => openSourceContinue(row.source)}
                              />
                            {:else}
                              <SourceContinueCard item={row.item} onOpen={openSourceContinue} />
                            {/if}
                          </div>
                        {/each}
                      {/if}
                    </Shelf>
                  </section>
                {/if}

                <Rail
                  title="Top 10 Today"
                  state={trendingRail}
                  rank={true}
                  isInLibrary={isInLibrary}
                  onOpen={(item) => void openDetail(item)}
                  onToggleLibrary={toggleLibrary}
                  onPlay={openPlay}
                  onRetry={() => void loadRail("trending", "trending", 1)}
                />

                <Rail
                  title="Trending Now"
                  state={rails.trendingMore}
                  isInLibrary={isInLibrary}
                  onOpen={(item) => void openDetail(item)}
                  onToggleLibrary={toggleLibrary}
                  onPlay={openPlay}
                  onRetry={() => void loadRail("trendingMore", "trending", 2)}
                />

                <Rail
                  title="Coming Soon"
                  state={rails.upcoming}
                  isInLibrary={isInLibrary}
                  onOpen={(item) => void openDetail(item)}
                  onToggleLibrary={toggleLibrary}
                  onPlay={openPlay}
                  onRetry={() => void loadRail("upcoming", "upcoming", 1)}
                />

                <Rail
                  title="Top Airing"
                  state={rails.airing}
                  isInLibrary={isInLibrary}
                  onOpen={(item) => void openDetail(item)}
                  onToggleLibrary={toggleLibrary}
                  onPlay={openPlay}
                  onRetry={() => void loadRail("airing", "airing", 1)}
                />

                <Rail
                  title="Top Rated"
                  state={rails.top}
                  isInLibrary={isInLibrary}
                  onOpen={(item) => void openDetail(item)}
                  onToggleLibrary={toggleLibrary}
                  onPlay={openPlay}
                  onRetry={() => void loadRail("top", "top", 1)}
                />

                <Rail
                  title="Popular"
                  state={rails.popular}
                  isInLibrary={isInLibrary}
                  onOpen={(item) => void openDetail(item)}
                  onToggleLibrary={toggleLibrary}
                  onPlay={openPlay}
                  onRetry={() => void loadRail("popular", "popular", 1)}
                />

                {#if library.length > 0}
                  <Shelf
                    title="Library Preview"
                    count={library.length}
                    onSeeAll={() => goView("library")}
                  >
                    {#each library.slice(0, 10) as entry (entry.media.id)}
                      <div class="w-[164px] lg:w-[184px] shrink-0">
                        <PosterCard
                          item={entry.media}
                          inLibrary={true}
                          progress={entry.media.episodes ? { watched: entry.watchedCount, total: entry.media.episodes } : null}
                          onOpen={(entryMedia) => void openDetail(entryMedia)}
                          onToggleLibrary={toggleLibrary}
                          onPlay={openPlay}
                        />
                      </div>
                    {/each}
                  </Shelf>
                {/if}
              </div>
            {/if}
          {:else if view === "library"}
            {#if libraryLoading && library.length === 0 && sourceLibraryItems.length === 0}
              <div class="p-6 md:p-8 max-w-[1440px] mx-auto">
                <Skeleton kind="grid" />
              </div>
            {:else}
              <LibraryView
                items={library}
                sourceItems={sourceLibraryItems}
                loading={libraryLoading}
                error={libraryError}
                onRetry={() => void refreshLibrary()}
                onOpen={(item) => void openDetail(item)}
                onOpenSource={openSourceShow}
                onResumeSource={resumeSource}
                onToggleLibrary={toggleLibrary}
                onPlay={openPlay}
                onBrowse={() => goView("browse")}
                onOpenSearch={openPalette}
                onOpenSources={() => goView("sources")}
                onSetStatus={(item, status) => void setStatus(item, status)}
              />
            {/if}
          {:else if view === "schedule"}
            <ScheduleView
              items={schedule}
              library={library}
              sourceItems={sourceLibraryItems}
              loading={scheduleLoading}
              error={scheduleError}
              warning={scheduleWarning}
              fetchedAt={scheduleFetchedAt}
              stale={scheduleStale}
              online={online}
              onRefresh={() => void refreshSchedule(true)}
              onOpen={(item) => void openDetail(item)}
              onOpenSource={openSourceShow}
              onBrowse={() => goView("browse")}
              onSearch={openPalette}
            />
          {:else if view === "history"}
            <HistoryView
              items={historyItems}
              sourceItems={sourceHistoryItems}
              loading={historyLoading}
              error={historyError}
              onRetry={() => {
                void refreshHistory();
                void refreshSourceHistory();
              }}
              onOpen={(item) => void openDetail(item)}
              onOpenSource={openSourceShow}
              onBrowse={() => goView("browse")}
              onClearHistory={clearHistory}
              onDeleteHistory={removeHistoryEntry}
              onDeleteSourceHistory={removeSourceHistoryEntry}
            />
          {:else if view === "sources"}
            <SourcesView boot={boot} notify={notify} onBrowse={openExtension} />
          {:else}
            <SettingsView
              boot={boot}
              incognito={incognito}
              onToggleIncognito={toggleIncognito}
              onOpenShortcuts={() => (shortcutsOpen = true)}
              onOpenLicenses={() => (licensesOpen = true)}
              onOpenOnboarding={() => (onboardingOpen = true)}
              onDataChanged={() => {
                void refreshLibrary();
                void refreshContinue();
                void refreshHistory();
                void refreshSourceActivity();
              }}
            />
          {/if}
            </div>
          {/key}
          {#snippet failed(error, reset)}
            <div class="flex-1">
              <FatalState
                code="VIEW_CRASHED"
                title="This view crashed"
                message="The app shell is still running. The failed view was stopped before it could spread. Reset it and keep browsing."
                detail={errorText(error)}
                retryLabel="Reset view"
                onRetry={reset}
              />
            </div>
          {/snippet}
        </svelte:boundary>
      {/if}
        <AppFooter onOpenLicenses={() => (licensesOpen = true)} />
      </div>
    </main>
  </div>
</div>

<SearchPalette
  open={paletteOpen}
  incognito={incognito}
  library={library}
  sourceLibrary={sourceLibraryItems}
  onClose={closePalette}
  onOpen={(item) => void openDetail(item)}
  onPlay={openPlay}
  onOpenSource={openSourceShow}
  onNav={goView}
  onOpenShortcuts={() => (shortcutsOpen = true)}
/>

{#if onboardingOpen}
  <GettingStarted
    onClose={closeOnboarding}
    onSources={onboardingSources}
    onBrowse={onboardingBrowse}
  />
{/if}

<ShortcutsDialog open={shortcutsOpen} onClose={() => (shortcutsOpen = false)} />

{#if sourceMatchMedia}
  <SourceMatchDialog
    media={sourceMatchMedia}
    onClose={() => (sourceMatchMedia = null)}
    onOpen={(extension, item) => {
      const linked = sourceMatchMedia;
      sourceBrowser = {
        extension: { id: extension.id, name: extension.name },
        initialShow: item,
        anilistMedia: linked ?? undefined,
        anilistId: linked?.id ?? null,
      };
      sourceMatchMedia = null;
    }}
    onOpenSources={() => {
      sourceMatchMedia = null;
      goView("sources");
    }}
  />
{/if}

{#if sourceBrowser}
  <SourceBrowser
    extension={sourceBrowser.extension}
    initialShow={sourceBrowser.initialShow}
    initialEpisodeId={sourceBrowser.resumeEpisodeId}
    anilistMedia={sourceBrowser.anilistMedia}
    anilistId={sourceBrowser.anilistId}
    sourceLibrary={sourceLibraryItems}
    incognito={incognito}
    onLibraryChanged={() =>
      void Promise.all([
        refreshSourceActivity(),
        refreshLibrary(),
        refreshContinue(),
        refreshHistory(),
      ])}
    onClose={() => (sourceBrowser = null)}
    notify={notify}
  />
{/if}

{#if licensesOpen}
  <LicensesDialog onClose={() => (licensesOpen = false)} />
{/if}

<Toasts toasts={toasts} onDismiss={dismissToast} />

<style>
  .skip-link {
    position: fixed;
    top: -64px;
    left: 88px;
    z-index: 80;
    padding: 8px 14px;
    border-radius: 10px;
    background: var(--volt-500);
    color: #ffffff;
    font-size: 13px;
    font-weight: 600;
    transition: top 150ms var(--ease-out);
  }

  .skip-link:focus {
    top: 48px;
  }

  #content:focus {
    outline: none;
  }

  #content:focus-visible {
    outline: 2px solid var(--volt-400);
    outline-offset: -2px;
  }
</style>
