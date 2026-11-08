<script lang="ts">
  import { onMount } from "svelte";
  import ContinueCard from "./lib/ContinueCard.svelte";
  import DetailPage from "./lib/DetailPage.svelte";
  import HeroCarousel from "./lib/HeroCarousel.svelte";
  import LibraryView from "./lib/LibraryView.svelte";
  import NavRail from "./lib/NavRail.svelte";
  import PosterCard from "./lib/PosterCard.svelte";
  import Rail from "./lib/Rail.svelte";
  import SearchPalette from "./lib/SearchPalette.svelte";
  import SettingsView from "./lib/SettingsView.svelte";
  import Shelf from "./lib/Shelf.svelte";
  import ShortcutsDialog from "./lib/ShortcutsDialog.svelte";
  import Skeleton from "./lib/Skeleton.svelte";
  import SourcesView from "./lib/SourcesView.svelte";
  import TitleBar from "./lib/TitleBar.svelte";
  import Toasts from "./lib/Toasts.svelte";
  import {
    addToLibrary,
    bootSnapshot,
    continueWatching,
    detail as fetchDetail,
    listLibrary,
    removeFromLibrary,
    shelf,
    watchedList,
    watchedMarkAll,
    watchedSet,
    type ShelfKind,
  } from "./lib/api";
  import { displayTitle, isRateLimitMessage } from "./lib/format";
  import type { Boot, ContinueItem, LibraryItem, Media, RailState, Toast, ToastTone, View } from "./lib/types";

  let title = $state("Kaminari");
  let boot = $state<Boot | null>(null);
  let library = $state<LibraryItem[]>([]);
  let libraryLoading = $state(true);
  let rails = $state<Record<string, RailState>>({
    trending: { items: [], loading: true, error: null },
    trendingMore: { items: [], loading: true, error: null },
    airing: { items: [], loading: true, error: null },
    top: { items: [], loading: true, error: null },
    popular: { items: [], loading: true, error: null },
  });
  let view = $state<View>("browse");
  let detail = $state<Media | null>(null);
  let detailError = $state<string | null>(null);
  let playNote = $state<string | null>(null);
  let watchedDetail = $state<number[]>([]);
  let continueItems = $state<ContinueItem[]>([]);
  let paletteOpen = $state(false);
  let shortcutsOpen = $state(false);
  let toasts = $state<Toast[]>([]);
  let toastSeq = $state(0);
  let lastFocus = $state<HTMLElement | null>(null);
  let online = $state(typeof navigator !== "undefined" ? navigator.onLine : true);

  const PLAY_NOTE =
    "Playback needs a source extension. AniList gives us the catalogue, not the stream. The extension host lands in P1.";

  function notify(text: string, tone: ToastTone = "info") {
    toastSeq += 1;
    const id = toastSeq;
    toasts = [...toasts.slice(-2), { id, text, tone }];
    window.setTimeout(() => dismissToast(id), 4200);
  }

  function dismissToast(id: number) {
    toasts = toasts.filter((toast) => toast.id !== id);
  }

  function isInLibrary(id: number): boolean {
    return library.some((entry) => entry.media.id === id);
  }

  function openPalette() {
    lastFocus = document.activeElement as HTMLElement | null;
    paletteOpen = true;
  }

  function closePalette() {
    paletteOpen = false;
    lastFocus?.focus?.();
  }

  function goView(next: View) {
    detail = null;
    detailError = null;
    playNote = null;
    watchedDetail = [];
    view = next;
  }

  async function refreshContinue() {
    try {
      continueItems = await continueWatching();
    } catch {
      continueItems = [];
    }
  }

  async function refreshLibrary() {
    try {
      library = await listLibrary();
    } catch {
      // An empty library is a valid first run. Toast only on explicit actions.
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
    ]);
  }

  function retryDetail() {
    if (detail) void openDetail(detail, { keepNote: true });
  }

  async function openDetail(item: Media, options: { keepNote?: boolean } = {}) {
    if (!options.keepNote) playNote = null;
    detailError = null;
    detail = item;
    try {
      detail = await fetchDetail(item.id);
    } catch (e) {
      detailError = e instanceof Error ? e.message : String(e);
    }
    try {
      watchedDetail = await watchedList(item.id);
    } catch {
      watchedDetail = [];
    }
  }

  function openPlay(item: Media) {
    playNote = PLAY_NOTE;
    notify("Source playback arrives in P1. Details are live now.", "info");
    void openDetail(item, { keepNote: true });
  }

  async function toggleLibrary(item: Media) {
    const name = displayTitle(item);
    try {
      if (isInLibrary(item.id)) {
        await removeFromLibrary(item.id);
        notify(`Removed ${name} from your library.`, "success");
      } else {
        await addToLibrary(item, "watching");
        notify(`Saved ${name} to your library.`, "success");
      }
      await refreshLibrary();
      await refreshContinue();
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Library change failed: ${message}`, "error");
    }
  }

  async function toggleWatched(episode: number) {
    if (!detail) return;
    const seen = watchedDetail.includes(episode);
    try {
      watchedDetail = await watchedSet(detail, episode, !seen);
      await Promise.all([refreshLibrary(), refreshContinue()]);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Episode update failed: ${message}`, "error");
    }
  }

  async function markAllWatched(seen: boolean) {
    if (!detail) return;
    const through = detail.episodes ?? 0;
    if (through <= 0) return;
    try {
      watchedDetail = await watchedMarkAll(detail, through, seen);
      await Promise.all([refreshLibrary(), refreshContinue()]);
      notify(
        seen ? `Marked all ${through} episodes watched.` : "Cleared watched episodes.",
        "success",
      );
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Episode update failed: ${message}`, "error");
    }
  }

  function closeDetail() {
    detail = null;
    detailError = null;
    playNote = null;
  }

  function typingTarget(event: KeyboardEvent): boolean {
    const target = event.target as HTMLElement | null;
    if (!target) return false;
    const tag = target.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.isContentEditable;
  }

  onMount(() => {
    void (async () => {
      try {
        boot = await bootSnapshot();
        title = boot.title;
      } catch {
        // Boot snapshot is informational. Shelves still load.
      }
      await refreshLibrary();
      await refreshContinue();
      await loadBrowse();
    })();
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
        if (shortcutsOpen) shortcutsOpen = false;
        else if (detail) closeDetail();
      } else if (event.key >= "1" && event.key <= "4") {
        const next: View[] = ["browse", "library", "sources", "settings"];
        const pick = next[Number(event.key) - 1];
        if (pick) goView(pick);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const trendingRail = $derived(rails.trending);
</script>

<div class="h-screen flex flex-col">
  <a
    href="#content"
    class="sr-only focus:not-sr-only focus:absolute focus:z-[60] focus:px-3 focus:py-2 focus:text-sm focus:rounded-lg"
    style="focus:background: var(--volt-500);"
  >
    Skip to content
  </a>
  <TitleBar title={title} onOpenPalette={openPalette} />

  <div class="flex flex-1 min-h-0">
    <NavRail view={view} onNav={goView} />

    <!-- The skip link focuses this region without adding it to the tab order. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <main id="content" class="ambient flex-1 overflow-y-auto min-w-0" tabindex={-1}>
      {#if !online}
        <div
          class="px-4 py-2 text-sm text-center"
          style="background: rgb(251 191 36 / 0.12); border-bottom: 1px solid rgb(251 191 36 / 0.35); color: var(--paper-50);"
          role="status"
        >
          You are offline. Showing cached titles. Search and detail refresh will fail until you reconnect.
        </div>
      {/if}

      {#if detail}
        {#if detailError}
          <div
            class="mx-6 md:mx-8 mt-6 px-4 py-3 rounded-xl text-sm"
            style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
            role="alert"
          >
            Full details did not load ({detailError}). Showing the cached card.
            <button class="btn-primary ml-3 px-3 py-1 text-xs" onclick={retryDetail}>Retry</button>
          </div>
        {/if}
        <DetailPage
          media={detail}
          inLibrary={isInLibrary(detail.id)}
          playNote={playNote}
          watched={watchedDetail}
          onBack={closeDetail}
          onToggleLibrary={toggleLibrary}
          onPlay={openPlay}
          onToggleWatched={(episode) => void toggleWatched(episode)}
          onMarkAll={(seen) => void markAllWatched(seen)}
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
            <div
              class="max-w-xl p-5 rounded-2xl"
              style="background: var(--void-800); border: 1px solid var(--line);"
              role="alert"
            >
              <h2 class="text-lg font-semibold" style="font-family: var(--font-display);">
                AniList did not answer
              </h2>
              <p class="text-sm mt-2" style="color: var(--paper-300);">{trendingRail.error}</p>
              <p class="text-xs mt-2" style="color: var(--paper-500);">
                {#if isRateLimitMessage(trendingRail.error)}
                  This is the public rate limit. It clears in under a minute. Other shelves load on their own.
                {:else}
                  Check your connection, then retry.
                {/if}
              </p>
              <button
                class="btn-primary mt-4 px-3 py-1.5 text-sm"
                onclick={() => void loadRail("trending", "trending", 1)}
              >
                Retry
              </button>
            </div>
          </div>
        {:else if trendingRail.items.length === 0}
          <div class="p-6 md:p-8 max-w-[1440px] mx-auto">
            <div
              class="max-w-xl p-5 rounded-2xl text-sm"
              style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
            >
              AniList returned an empty shelf. Try again in a moment.
              <button
                class="btn-primary ml-3 px-3 py-1 text-xs"
                onclick={() => void loadRail("trending", "trending", 1)}
              >
                Reload
              </button>
            </div>
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
            {#if library.length === 0}
              <div
                class="flex items-center gap-3 rounded-xl px-4 py-3 text-sm"
                style="background: var(--void-900); border: 1px solid var(--line);"
              >
                <p class="flex-1" style="color: var(--paper-300);">
                  New here? Save posters to build your library.
                </p>
                <button class="btn-ghost px-3 py-1.5 text-xs shrink-0" onclick={openPalette}>
                  Search (Ctrl K)
                </button>
              </div>
            {:else}
              <section aria-label="Continue watching">
                <Shelf title="Continue Watching" count={continueItems.length}>
                  {#if continueItems.length === 0}
                    <div
                      class="w-full rounded-xl px-4 py-3 text-sm"
                      style="background: var(--void-900); border: 1px solid var(--line); color: var(--paper-500);"
                    >
                      Nothing in progress yet. Mark an episode watched and this row fills itself.
                    </div>
                  {:else}
                    {#each continueItems as entry (entry.media.id)}
                      <div class="w-[280px] md:w-[320px] shrink-0">
                        <ContinueCard item={entry} onOpen={(item) => void openDetail(item)} />
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
                  <div class="w-[160px] md:w-[180px] shrink-0">
                    <PosterCard
                      item={entry.media}
                      inLibrary={true}
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
        {#if libraryLoading && library.length === 0}
          <div class="p-6 md:p-8 max-w-[1440px] mx-auto">
            <Skeleton kind="grid" />
          </div>
        {:else}
          <LibraryView
            items={library}
            onOpen={(item) => void openDetail(item)}
            onToggleLibrary={toggleLibrary}
            onPlay={openPlay}
            onBrowse={() => goView("browse")}
          />
        {/if}
      {:else if view === "sources"}
        <SourcesView boot={boot} />
      {:else}
        <SettingsView boot={boot} />
      {/if}
    </main>
  </div>
</div>

<SearchPalette
  open={paletteOpen}
  library={library}
  onClose={closePalette}
  onOpen={(item) => void openDetail(item)}
  notify={notify}
/>

<ShortcutsDialog open={shortcutsOpen} onClose={() => (shortcutsOpen = false)} />

<Toasts toasts={toasts} onDismiss={dismissToast} />

<style>
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
  }
  .sr-only:focus {
    position: fixed;
    top: 48px;
    left: 88px;
    width: auto;
    height: auto;
    clip: auto;
    background: var(--volt-500);
    color: var(--paper-50);
  }
</style>
