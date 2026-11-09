<script lang="ts">
  import { onMount } from "svelte";
  import Callout from "./Callout.svelte";
  import EmptyState from "./EmptyState.svelte";
  import Icon from "./Icon.svelte";
  import PlayerView from "./PlayerView.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import {
    jsExtensionDetail,
    jsExtensionEpisodes,
    jsExtensionResolve,
    jsExtensionTest,
    playerPlay,
    proxyRegister,
    proxyRevoke,
    proxyStatus,
    sourceLibraryAdd,
    sourceLibraryRemove,
    sourceProgressGet,
    sourceProgressSet,
  } from "./api";
  import { isHlsStream, proxyFlagEnabled, shouldProxy } from "./streamProxy";
  import type {
    JsEpisode,
    JsSearchItem,
    JsShowDetail,
    JsStream,
    SourceExtensionRef,
    SourceLibraryItem,
    SourceProgress,
  } from "./types";

  let {
    extension,
    initialShow = null,
    initialEpisodeId = null,
    sourceLibrary = [],
    onLibraryChanged,
    onClose,
    notify,
  }: {
    extension: SourceExtensionRef;
    initialShow?: JsSearchItem | null;
    initialEpisodeId?: string | null;
    sourceLibrary?: SourceLibraryItem[];
    onLibraryChanged?: () => void;
    onClose: () => void;
    notify: (text: string, tone?: "info" | "success" | "error") => void;
  } = $props();

  let query = $state("");
  let results = $state<JsSearchItem[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let selected = $state<JsShowDetail | null>(null);
  let episodes = $state<JsEpisode[]>([]);
  let showLoading = $state(false);
  let showError = $state<string | null>(null);
  let streams = $state<Record<string, JsStream[]>>({});
  let resolving = $state<string | null>(null);
  let streamError = $state<string | null>(null);
  let activePlayback = $state<{ stream: JsStream; episode: JsEpisode; startSec: number } | null>(null);
  let progress = $state<SourceProgress[]>([]);
  let searchInput: HTMLInputElement | null = $state(null);
  let dialog: HTMLDivElement | null = $state(null);
  let closeButton: HTMLButtonElement | null = $state(null);
  let previousFocus: HTMLElement | null = null;
  let autoPlayed = false;
  let progressSave: Promise<void> = Promise.resolve();

  const inSourceLibrary = $derived(
    selected
      ? sourceLibrary.some(
          (item) =>
            item.extensionId === extension.id && item.showId === selected?.id,
        )
      : false,
  );

  onMount(() => {
    previousFocus = document.activeElement as HTMLElement | null;
    document.body.dataset.overlay = "source";
    requestAnimationFrame(() => {
      if (searchInput?.offsetParent) searchInput.focus();
      else closeButton?.focus();
    });
    void (async () => {
      if (initialShow) {
        await openShow(initialShow);
      } else {
        await runSearch();
      }
    })();
    searchInput?.focus();

    return () => {
      delete document.body.dataset.overlay;
      previousFocus?.focus?.();
    };
  });

  $effect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || activePlayback) return;
      event.preventDefault();
      onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function trapTab(event: KeyboardEvent) {
    if (event.key !== "Tab" || !dialog) return;
    const items = [...dialog.querySelectorAll("button, input, select")].filter(
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

  async function runSearch() {
    loading = true;
    error = null;
    try {
      const result = await jsExtensionTest(extension.id, query.trim());
      results = result.items ?? [];
      if (results.length === 0) {
        error = query.trim()
          ? `No results for "${query.trim()}".`
          : "The source returned an empty shelf. Try a search term.";
      }
    } catch (e) {
      results = [];
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function openShow(item: JsSearchItem) {
    selected = {
      id: item.id,
      title: item.title,
      cover: item.cover ?? null,
      year: item.year ?? null,
    };
    episodes = [];
    streams = {};
    showError = null;
    streamError = null;
    showLoading = true;
    try {
      selected = await jsExtensionDetail(extension.id, item.id);
    } catch (e) {
      showError = e instanceof Error ? e.message : String(e);
    }
    try {
      episodes = await jsExtensionEpisodes(extension.id, item.id);
    } catch (e) {
      if (!showError) {
        showError = e instanceof Error ? e.message : String(e);
      }
    } finally {
      showLoading = false;
    }
    try {
      progress = await sourceProgressGet(extension.id, item.id);
    } catch {
      progress = [];
    }

    // A Continue card carries the episode id, so opening it should finish the
    // job: resolve that episode and resume at the saved position.
    if (initialEpisodeId && !autoPlayed && initialShow?.id === item.id) {
      autoPlayed = true;
      const episode = episodes.find((row) => row.id === initialEpisodeId);
      if (episode) {
        const list = await resolve(episode);
        if (list.length > 0) await playStream(list[0], episode);
      }
    }
  }

  function progressFor(episodeId: string): SourceProgress | undefined {
    return progress.find((row) => row.episodeId === episodeId);
  }

  function resumeFor(episodeId: string): number {
    const row = progressFor(episodeId);
    if (!row || row.watched || row.positionSec <= 0) return 0;
    if (row.durationSec > 0 && row.positionSec / row.durationSec >= 0.95) return 0;
    return row.positionSec;
  }

  function episodeAction(episode: JsEpisode): string {
    const row = progressFor(episode.id);
    if (row?.watched) return "Rewatch";
    return resumeFor(episode.id) > 0 ? "Resume" : "Play";
  }

  async function toggleSourceLibrary() {
    if (!selected) return;
    try {
      if (inSourceLibrary) {
        await sourceLibraryRemove(extension.id, selected.id);
        notify(`Removed ${selected.title} from the source library.`, "success");
      } else {
        await sourceLibraryAdd({
          extensionId: extension.id,
          showId: selected.id,
          title: selected.title,
          cover: selected.cover ?? null,
          year: selected.year ?? null,
          status: selected.status ?? null,
        });
        notify(`Saved ${selected.title} to the source library.`, "success");
      }
      onLibraryChanged?.();
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Source library change failed: ${message}`, "error");
    }
  }

  async function saveProgress(position: number, duration: number, watched: boolean) {
    if (!selected || !activePlayback) return;
    const episode = activePlayback.episode;
    try {
      await sourceProgressSet({
        extensionId: extension.id,
        showId: selected.id,
        episodeId: episode.id,
        episodeNumber: episode.number,
        episodeTitle: episode.title ?? null,
        positionSec: position,
        durationSec: duration,
        watched,
      });
    } catch {
      // Progress is best effort. Playback stays up.
    }
  }

  async function closePlayer() {
    const proxyId = activePlayback?.stream.proxyId ?? null;
    activePlayback = null;
    await progressSave;
    if (proxyId) {
      try {
        await proxyRevoke(proxyId);
      } catch {
        // Revoke is best effort. The entry also expires on its own.
      }
    }
    if (!selected) return;
    try {
      progress = await sourceProgressGet(extension.id, selected.id);
    } catch {
      // Keep the old progress on a read failure.
    }
    onLibraryChanged?.();
  }

  async function resolve(episode: JsEpisode): Promise<JsStream[]> {
    resolving = episode.id;
    streamError = null;
    try {
      const list = await jsExtensionResolve(extension.id, episode.id);
      streams = { ...streams, [episode.id]: list };
      if (!list || list.length === 0) {
        streamError = "The source returned no streams for this episode.";
      }
      return list ?? [];
    } catch (e) {
      streamError = e instanceof Error ? e.message : String(e);
      return [];
    } finally {
      resolving = null;
    }
  }

  async function copyStream(url: string) {
    try {
      await navigator.clipboard.writeText(url);
      notify("Stream URL copied.", "success");
    } catch {
      notify("Could not copy the URL.", "error");
    }
  }

  async function playStream(stream: JsStream, episode: JsEpisode) {
    let prepared = stream;
    if (shouldProxy(stream) && proxyFlagEnabled()) {
      try {
        const status = await proxyStatus();
        if (status.enabled) {
          const registered = await proxyRegister(extension.id, {
            url: stream.url,
            kind: isHlsStream(stream) ? "hls" : "mp4",
            headers: stream.headers ?? null,
          });
          prepared = {
            ...stream,
            proxiedUrl: registered.proxyUrl,
            proxyId: registered.opaqueId,
          };
        }
      } catch {
        notify("The protected stream could not be proxied. Trying it directly.", "info");
      }
    }
    activePlayback = { stream: prepared, episode, startSec: resumeFor(episode.id) };
  }

  async function playEpisode(episode: JsEpisode) {
    const cached = streams[episode.id];
    const list = cached && cached.length > 0 ? cached : await resolve(episode);
    if (list.length > 0) await playStream(list[0], episode);
  }

  function toggleStreams(episode: JsEpisode) {
    if (streams[episode.id]) {
      const next = { ...streams };
      delete next[episode.id];
      streams = next;
      return;
    }
    void resolve(episode);
  }

  async function openExternal(stream: JsStream, episode: JsEpisode) {
    try {
      const info = await playerPlay({
        url: stream.url,
        title: `${selected?.title ?? "Kaminari"} Â· Episode ${episode.number}`,
        headers: stream.headers ?? undefined,
        subtitles: stream.subtitles ?? undefined,
      });
      notify(`Opening mpv from ${info.mpvPath}.`, "success");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Could not open mpv: ${message}`, "error");
    }
  }

  function backToResults() {
    selected = null;
    episodes = [];
    streams = {};
    showError = null;
    streamError = null;
  }

  function duration(seconds?: number | null): string {
    if (!seconds || seconds <= 0) return "";
    const minutes = Math.floor(seconds / 60);
    const rest = seconds % 60;
    return `${minutes}:${String(rest).padStart(2, "0")}`;
  }

  function formatTime(value: number): string {
    if (!Number.isFinite(value) || value < 0) return "0:00";
    const hours = Math.floor(value / 3600);
    const minutes = Math.floor((value % 3600) / 60);
    const seconds = Math.floor(value % 60);
    return hours > 0
      ? `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`
      : `${minutes}:${String(seconds).padStart(2, "0")}`;
  }

  function clean(text?: string | null): string {
    if (!text) return "";
    return text
      .replace(/<br\s*\/?>/gi, " ")
      .replace(/<[^>]+>/g, " ")
      .replace(/&quot;/g, '"')
      .replace(/&#0?39;/g, "'")
      .replace(/&amp;/g, "&")
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/\s+/g, " ")
      .trim();
  }

  function hostOf(url: string): string {
    try {
      return new URL(url).host;
    } catch {
      return url.slice(0, 42);
    }
  }
</script>

<div
  bind:this={dialog}
  class="fixed inset-0 z-40 flex flex-col"
  style="background: radial-gradient(900px 500px at 20% -10%, var(--accent-wash), transparent 60%), var(--void-950);"
  role="dialog"
  aria-modal="true"
  aria-label="{extension.name} source browser"
  tabindex={-1}
  onkeydown={trapTab}
>
  <header class="source-browser-head h-14 shrink-0 flex items-center gap-3 px-4 glass-strong">
    <button bind:this={closeButton} class="btn-ghost px-2.5 py-2 text-xs" onclick={onClose} aria-label="Close source browser">
      <Icon name="close" size={15} />
    </button>
    <div class="source-tile" style="width: 34px; height: 34px; font-size: 10px;" aria-hidden="true">JS</div>
    <div class="min-w-0">
      <p class="text-sm font-semibold truncate">{extension.name}</p>
      <p class="text-[10px] truncate" style="color: var(--paper-450); font-family: var(--font-mono);">
        {extension.id}
      </p>
    </div>
    <form
      class="ml-auto flex items-center gap-2"
      onsubmit={(event) => {
        event.preventDefault();
        void runSearch();
      }}
    >
      <label class="relative hidden sm:block">
        <span class="sr-only">Search this source</span>
        <span class="absolute left-3 top-1/2 -translate-y-1/2" style="color: var(--paper-450);">
          <Icon name="search" size={13} />
        </span>
        <input
          bind:this={searchInput}
          bind:value={query}
          placeholder="Search this source"
          class="h-8 w-[240px] rounded-lg pl-8 pr-3 text-sm outline-none"
          style="background: var(--input-bg); border: 1px solid var(--line); color: var(--text-1);"
          aria-label="Search this source"
        />
      </label>
      <button type="submit" class="btn-primary px-3 py-1.5 text-[11px]" disabled={loading}>
        {loading ? "Searching" : "Search"}
      </button>
    </form>
  </header>

  <main class="flex-1 overflow-y-auto">
    {#if selected}
      <div class="max-w-[1200px] mx-auto p-6 md:p-8">
        <div class="flex flex-wrap items-center gap-2 mb-4">
          <button class="btn-ghost px-3 py-1.5 text-xs" onclick={backToResults}>
            <Icon name="arrowLeft" size={14} />
            Back to results
          </button>
          <button
            class={inSourceLibrary ? "btn-ghost" : "btn-primary"}
            class:px-3={true}
            class:py-1.5={true}
            class:text-xs={true}
            onclick={() => void toggleSourceLibrary()}
          >
            <Icon name={inSourceLibrary ? "check" : "plus"} size={14} />
            {inSourceLibrary ? "In Library" : "Library"}
          </button>
        </div>

        <div class="relative overflow-hidden rounded-3xl" style="border: 1px solid var(--line-strong); box-shadow: var(--shadow-pop);">
          {#if selected.banner || selected.cover}
            <img src={selected.banner || selected.cover} alt="" class="absolute inset-0 w-full h-full object-cover blur-2xl scale-110 opacity-40" />
          {/if}
          <div class="source-hero-scrim absolute inset-0" aria-hidden="true"></div>
          <div class="relative flex flex-col sm:flex-row gap-6 p-6 md:p-8">
            <div
              class="w-[150px] sm:w-[190px] shrink-0 aspect-[2/3] rounded-2xl overflow-hidden"
              style="background: var(--void-800); border: 1px solid var(--line-strong); box-shadow: 0 20px 50px rgb(0 0 0 / 0.55);"
            >
              {#if selected.cover}
                <img src={selected.cover} alt="" class="w-full h-full object-cover" />
              {:else}
                <div class="w-full h-full grid place-items-center">
                  <span class="text-4xl font-bold" style="color: var(--paper-450); font-family: var(--font-display);">{selected.title.slice(0, 1)}</span>
                </div>
              {/if}
            </div>
            <div class="min-w-0 flex-1 self-end">
              <p class="eyebrow">Source result</p>
              <h1 class="mt-1 leading-[1.04] line-clamp-2" style="font-family: var(--font-display); font-size: clamp(28px, 3.6vw, 46px); font-weight: 750; letter-spacing: -0.03em;">
                {selected.title}
              </h1>
              <div class="flex flex-wrap items-center gap-2 mt-2.5">
                {#if selected.year}<span class="chip text-[11px] px-2.5 py-1">{selected.year}</span>{/if}
                {#if selected.status}<span class="chip text-[11px] px-2.5 py-1">{selected.status}</span>{/if}
                <span class="chip chip-volt text-[11px] px-2.5 py-1">
                  <Icon name="package" size={11} />
                  {extension.name}
                </span>
              </div>
              {#if selected.genres?.length}
                <div class="flex flex-wrap gap-1.5 mt-2.5">
                  {#each selected.genres as genre}
                    <span class="chip text-[10px] px-2 py-0.5">{genre}</span>
                  {/each}
                </div>
              {/if}
              <p class="text-sm mt-3 leading-relaxed max-w-2xl line-clamp-3" style="color: var(--paper-300);">
                {clean(selected.synopsis) || "This source did not return a synopsis."}
              </p>
            </div>
          </div>
        </div>

        {#if showError}
          <Callout tone="danger" class="mt-4">{showError}</Callout>
        {/if}

        <div class="flex items-baseline gap-3 mt-8 mb-3">
          <h2 class="shelf-title">Episodes</h2>
          <span class="shelf-count">{episodes.length}</span>
          <span class="ml-auto text-[10px]" style="color: var(--paper-450); font-family: var(--font-mono);">
            {progress.filter((row) => row.watched).length} watched
          </span>
        </div>

        {#if showLoading}
          <div class="space-y-2">
            {#each Array(4) as _}
              <div class="surface p-3 flex items-center gap-3">
                <div class="shimmer w-24 h-14 rounded-lg shrink-0"></div>
                <div class="flex-1 space-y-2">
                  <div class="shimmer h-3 w-1/2 rounded"></div>
                  <div class="shimmer h-2 w-1/3 rounded"></div>
                </div>
                <div class="shimmer h-8 w-20 rounded-lg"></div>
              </div>
            {/each}
          </div>
        {:else if episodes.length === 0}
          <div class="surface px-4 py-5 text-sm flex items-center gap-3" style="color: var(--paper-300);">
            <span style="color: var(--paper-450);"><Icon name="info" size={16} /></span>
            This source returned no episodes for this title.
          </div>
        {:else}
          <div class="space-y-2">
            {#each episodes as episode (episode.id)}
              {@const saved = progressFor(episode.id)}
              <div class="surface overflow-hidden">
                <div class="flex items-center gap-3 p-3">
                  <div class="episode-thumb shrink-0">
                    {#if episode.thumbnail}
                      <img src={episode.thumbnail} alt="" loading="lazy" decoding="async" />
                    {:else if selected.banner || selected.cover}
                      <img
                        src={selected.banner || selected.cover}
                        alt=""
                        loading="lazy"
                        decoding="async"
                        class="is-fallback"
                      />
                    {:else}
                      <div class="episode-thumb-empty" aria-hidden="true"></div>
                    {/if}
                    <span class="episode-thumb-scrim" aria-hidden="true"></span>
                    <span class="episode-thumb-number">EP {episode.number}</span>
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2">
                      <p class="text-sm font-semibold truncate">
                        {episode.title || `Episode ${episode.number}`}
                      </p>
                      {#if saved?.watched}
                        <span class="chip chip-live text-[9px] px-1.5 py-0.5">Watched</span>
                      {/if}
                    </div>
                    <p class="text-[11px] mt-0.5" style="color: var(--paper-450); font-family: var(--font-mono);">
                      EP {episode.number}{duration(episode.durationSec) ? ` Â· ${duration(episode.durationSec)}` : ""}
                    </p>
                    {#if saved}
                      <div class="h-0.5 rounded-full mt-1.5 overflow-hidden" style="background: var(--void-700);" aria-hidden="true">
                        <div
                          class="h-full"
                          style="width: {saved.durationSec > 0
                            ? Math.min(100, (saved.positionSec / saved.durationSec) * 100)
                            : 0}%; background: var(--volt-400);"
                        ></div>
                      </div>
                      <p class="text-[10px] mt-0.5" style="color: var(--paper-450);">
                        {saved.watched
                          ? "Watched"
                          : saved.positionSec > 0
                            ? `Resume ${formatTime(saved.positionSec)}`
                            : ""}
                      </p>
                    {/if}
                  </div>
                  <div class="flex items-center gap-1.5 shrink-0">
                    <button
                      class="btn-primary px-3 py-1.5 text-[11px] shrink-0"
                      disabled={resolving === episode.id}
                      onclick={() => void playEpisode(episode)}
                      aria-label="{resolving === episode.id ? 'Resolving' : episodeAction(episode)} episode {episode.number}"
                    >
                      <Icon name="playCircle" size={13} />
                      {resolving === episode.id ? "Resolving" : episodeAction(episode)}
                    </button>
                    <button
                      class="btn-ghost px-2.5 py-1.5 text-[11px] shrink-0"
                      disabled={resolving === episode.id}
                      onclick={() => toggleStreams(episode)}
                      aria-label="{streams[episode.id] ? 'Hide' : 'Show'} streams for episode {episode.number}"
                    >
                      <Icon name="list" size={12} />
                      {streams[episode.id] ? "Hide" : "Streams"}
                    </button>
                  </div>
                </div>
                {#if resolving === episode.id}
                  <div class="px-3 pb-3">
                    <ProgressBar label="Resolving episode" indeterminate size="xs" />
                  </div>
                {/if}
                {#if streams[episode.id]}
                  <div class="px-3 pb-3 space-y-1.5" style="border-top: 1px solid var(--line); padding-top: 10px;">
                    {#each streams[episode.id] as stream}
                      <div class="flex items-center gap-2 px-2.5 py-2 rounded-lg" style="background: var(--well-bg); border: 1px solid var(--line);">
                        <span class="chip chip-volt text-[9px] px-1.5 py-0.5 shrink-0">
                          {stream.type.toUpperCase()}
                        </span>
                        {#if stream.quality}
                          <span class="text-[10px] font-semibold shrink-0" style="color: var(--paper-300);">{stream.quality}</span>
                        {/if}
                        <span class="text-[10px] truncate flex-1" style="color: var(--paper-450); font-family: var(--font-mono);" title={stream.url}>
                          {hostOf(stream.url)}
                        </span>
                        <button class="btn-primary px-2.5 py-1 text-[10px] shrink-0" onclick={() => void playStream(stream, episode)}>
                          <Icon name="play" size={10} />
                          {episodeAction(episode)}
                        </button>
                        <button class="btn-ghost px-2.5 py-1 text-[10px] shrink-0" onclick={() => void copyStream(stream.url)}>
                          <Icon name="copy" size={10} />
                          Copy
                        </button>
                        <button class="btn-ghost px-2.5 py-1 text-[10px] shrink-0" onclick={() => void openExternal(stream, episode)} aria-label="Open in mpv">
                          <Icon name="external" size={10} />
                        </button>
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
          {#if streamError}
            <Callout tone="danger" class="mt-3">{streamError}</Callout>
          {/if}
          <p class="text-xs mt-3" style="color: var(--paper-450);">
            Play starts the first stream in the in-app player. Streams lists every quality, with Copy
            and Open in mpv for the ones the WebView cannot decode.
          </p>
        {/if}
      </div>
    {:else}
      <div class="max-w-[1440px] mx-auto p-6 md:p-8">
        <div class="flex items-baseline gap-3 mb-4">
          <div>
            <p class="eyebrow">Source browser</p>
            <h2 class="shelf-title mt-1">Search results</h2>
          </div>
          <span class="shelf-count">{results.length}</span>
          <span class="ml-auto text-[10px]" style="color: var(--paper-450); font-family: var(--font-mono);">
            {extension.name}
          </span>
        </div>

        {#if loading}
          <ProgressBar label="Searching source" indeterminate class="mb-4" />
          <div class="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-6 gap-4">
            {#each Array(12) as _}
              <div class="space-y-2">
                <div class="aspect-[2/3] rounded-xl shimmer"></div>
                <div class="h-3 w-3/4 rounded shimmer"></div>
              </div>
            {/each}
          </div>
        {:else if error}
          <EmptyState
            compact
            icon="search"
            title={error.startsWith("No results") ? "No matches in this source" : "Source search failed"}
            body={error}
            actionLabel="Retry"
            actionIcon="refresh"
            onAction={() => void runSearch()}
          />
        {:else if results.length === 0}
          <EmptyState
            compact
            icon="search"
            title="Search this source"
            body="Type a title and run a search. Results come straight from the extension host."
            actionLabel="Search"
            onAction={() => void runSearch()}
          />
        {:else}
          <div class="stagger-grid grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-6 gap-4">
            {#each results as item (item.id)}
              <div>
                <button
                  class="poster-card group block w-full text-left"
                  onclick={() => void openShow(item)}
                  aria-label="Open {item.title}"
                >
                  <div class="poster-art aspect-[2/3]">
                    {#if item.cover}
                      <img src={item.cover} alt="" loading="lazy" decoding="async" class="w-full h-full object-cover" />
                    {:else}
                      <div class="w-full h-full grid place-items-center" style="background: radial-gradient(circle at 30% 20%, var(--accent-wash-strong), transparent 60%), var(--void-800);">
                        <span class="text-3xl font-bold" style="color: var(--paper-450); font-family: var(--font-display);">{item.title.slice(0, 1)}</span>
                      </div>
                    {/if}
                    <span class="poster-scrim" aria-hidden="true"></span>
                    <span class="poster-badge">Source</span>
                    {#if item.score}
                      <span class="poster-score">
                        <Icon name="star" size={9} />
                        {(item.score / 10).toFixed(1)}
                      </span>
                    {/if}
                  </div>
                  <div class="poster-body">
                    <p class="poster-title">{item.title}</p>
                    <p class="poster-meta">{item.year ?? "Source result"}{item.episodes ? ` Â· ${item.episodes} eps` : ""}</p>
                  </div>
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </main>
</div>

{#if activePlayback}
  <PlayerView
    stream={activePlayback.stream}
    title={selected?.title ?? "Source playback"}
    subtitle="Episode {activePlayback.episode.number}{activePlayback.episode.title
      ? ` Â· ${activePlayback.episode.title}`
      : ''}"
    startSec={activePlayback.startSec}
    onProgress={(position, duration, watched) => {
      progressSave = saveProgress(position, duration, watched);
    }}
    onClose={() => void closePlayer()}
    onOpenExternal={() =>
      void openExternal(activePlayback!.stream, activePlayback!.episode)}
    {notify}
  />
{/if}

<style>
  .source-browser-head {
    border: 0;
    border-bottom: 1px solid var(--line);
    box-shadow: none;
  }

  .episode-thumb {
    position: relative;
    width: 96px;
    height: 56px;
    overflow: hidden;
    border-radius: 10px;
    background: var(--void-700);
    border: 1px solid var(--line);
  }

  .episode-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .episode-thumb img.is-fallback {
    filter: saturate(0.72) brightness(0.9);
    transform: scale(1.06);
  }

  .episode-thumb-empty {
    width: 100%;
    height: 100%;
    background:
      radial-gradient(circle at 30% 20%, var(--accent-wash-strong), transparent 60%),
      var(--void-800);
  }

  .episode-thumb-scrim {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(0deg, rgb(8 8 15 / 0.82), transparent 62%),
      linear-gradient(180deg, rgb(8 8 15 / 0.28), transparent 34%);
    pointer-events: none;
  }

  .episode-thumb-number {
    position: absolute;
    left: 5px;
    bottom: 5px;
    display: inline-flex;
    align-items: center;
    height: 17px;
    padding: 0 5px;
    border-radius: 5px;
    background: rgb(8 8 15 / 0.72);
    border: 1px solid rgb(255 255 255 / 0.18);
    color: #f4f3ff;
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.04em;
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
  }
</style>
