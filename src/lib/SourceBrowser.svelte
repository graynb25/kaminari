<script lang="ts">
  import { onMount } from "svelte";
  import {
    jsExtensionDetail,
    jsExtensionEpisodes,
    jsExtensionResolve,
    jsExtensionTest,
  } from "./api";
  import type { JsEpisode, JsExtension, JsSearchItem, JsShowDetail, JsStream } from "./types";

  let {
    extension,
    onClose,
    notify,
  }: {
    extension: JsExtension;
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
  let searchInput: HTMLInputElement | null = $state(null);

  onMount(() => {
    void runSearch();
    searchInput?.focus();
  });

  $effect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

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
  }

  async function resolve(episode: JsEpisode) {
    resolving = episode.id;
    streamError = null;
    try {
      const list = await jsExtensionResolve(extension.id, episode.id);
      streams = { ...streams, [episode.id]: list };
      if (!list || list.length === 0) {
        streamError = "The source returned no streams for this episode.";
      }
    } catch (e) {
      streamError = e instanceof Error ? e.message : String(e);
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
</script>

<div
  class="fixed inset-0 z-40 flex flex-col"
  style="background: var(--void-950);"
  role="dialog"
  aria-modal="true"
  aria-label="{extension.name} source browser"
>
  <header
    class="h-12 shrink-0 flex items-center gap-3 px-4"
    style="background: var(--void-900); border-bottom: 1px solid var(--line);"
  >
    <button class="btn-ghost px-3 py-1.5 text-xs" onclick={onClose}>Close</button>
    <div class="min-w-0">
      <p class="text-sm font-semibold truncate">{extension.name}</p>
      <p class="text-[10px] truncate" style="color: var(--paper-500); font-family: var(--font-mono);">
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
      <input
        bind:this={searchInput}
        bind:value={query}
        placeholder="Search this source"
        class="h-8 w-[260px] rounded-lg px-3 text-sm outline-none"
        style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-50);"
        aria-label="Search this source"
      />
      <button type="submit" class="btn-primary px-3 py-1.5 text-xs" disabled={loading}>
        {loading ? "Searching." : "Search"}
      </button>
    </form>
  </header>

  <main class="flex-1 overflow-y-auto">
    {#if selected}
      <div class="max-w-[1200px] mx-auto p-6 md:p-8">
        <button class="btn-ghost px-3 py-1.5 text-xs mb-4" onclick={backToResults}>
          ← Back to results
        </button>

        {#if showError}
          <div
            class="rounded-xl px-4 py-3 text-sm mb-4"
            style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
            role="alert"
          >
            {showError}
          </div>
        {/if}

        <div class="flex flex-col sm:flex-row gap-6">
          <div
            class="w-[160px] sm:w-[200px] shrink-0 aspect-[2/3] rounded-2xl overflow-hidden"
            style="background: var(--void-800); border: 1px solid var(--line);"
          >
            {#if selected.cover}
              <img src={selected.cover} alt="" class="w-full h-full object-cover" />
            {/if}
          </div>
          <div class="min-w-0 flex-1">
            <p class="eyebrow">Source result</p>
            <h1
              class="mt-1 leading-tight"
              style="font-family: var(--font-display); font-size: clamp(26px, 3vw, 38px); font-weight: 700;"
            >
              {selected.title}
            </h1>
            <p class="text-sm mt-2" style="color: var(--paper-300);">
              {selected.year ?? ""}
              {#if selected.status} · {selected.status}{/if}
            </p>
            {#if selected.genres?.length}
              <div class="flex flex-wrap gap-1.5 mt-3">
                {#each selected.genres as genre}
                  <span class="chip text-xs px-2 py-0.5">{genre}</span>
                {/each}
              </div>
            {/if}
            <p class="text-sm mt-4 leading-relaxed max-w-2xl" style="color: var(--paper-300);">
              {selected.synopsis || "This source did not return a synopsis."}
            </p>
          </div>
        </div>

        <div class="flex items-baseline gap-3 mt-8 mb-3">
          <h2 class="text-lg font-semibold">Episodes</h2>
          <span class="text-sm" style="color: var(--paper-500);">{episodes.length}</span>
        </div>

        {#if showLoading}
          <p class="text-sm" style="color: var(--paper-500);">Reading metadata and episodes.</p>
        {:else if episodes.length === 0}
          <p class="text-sm" style="color: var(--paper-500);">
            This source returned no episodes for this title.
          </p>
        {:else}
          <div class="space-y-2">
            {#each episodes as episode (episode.id)}
              <div
                class="rounded-xl overflow-hidden"
                style="background: var(--void-900); border: 1px solid var(--line);"
              >
                <div class="flex items-center gap-3 px-3 py-2.5">
                  <span
                    class="w-8 h-8 grid place-items-center rounded-lg text-xs font-semibold shrink-0"
                    style="background: var(--void-800); color: var(--volt-300); font-family: var(--font-mono);"
                  >
                    {episode.number}
                  </span>
                  <div class="min-w-0 flex-1">
                    <p class="text-sm font-semibold truncate">
                      {episode.title || `Episode ${episode.number}`}
                    </p>
                    <p class="text-[11px]" style="color: var(--paper-500);">
                      {duration(episode.durationSec)}
                    </p>
                  </div>
                  <button
                    class="btn-primary px-3 py-1.5 text-xs shrink-0"
                    disabled={resolving === episode.id}
                    onclick={() => void resolve(episode)}
                  >
                    {resolving === episode.id ? "Resolving." : "Resolve"}
                  </button>
                </div>
                {#if streams[episode.id]}
                  <div class="px-3 pb-3 space-y-1.5">
                    {#each streams[episode.id] as stream}
                      <div
                        class="flex items-center gap-2 px-2 py-1.5 rounded-lg"
                        style="background: var(--void-800); border: 1px solid var(--line);"
                      >
                        <span class="text-[10px] font-semibold" style="color: var(--volt-300);">
                          {stream.type.toUpperCase()}
                          {#if stream.quality} · {stream.quality}{/if}
                        </span>
                        <span
                          class="text-[10px] truncate flex-1"
                          style="color: var(--paper-500); font-family: var(--font-mono);"
                          title={stream.url}
                        >
                          {stream.url}
                        </span>
                        <button
                          class="btn-ghost px-2 py-0.5 text-[10px] shrink-0"
                          onclick={() => void copyStream(stream.url)}
                        >
                          Copy
                        </button>
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
          {#if streamError}
            <div
              class="mt-3 rounded-lg px-3 py-2 text-xs"
              style="background: rgb(248 113 113 / 0.1); border: 1px solid rgb(248 113 113 / 0.35);"
              role="alert"
            >
              {streamError}
            </div>
          {/if}
          <p class="text-xs mt-3" style="color: var(--paper-500);">
            Resolve returns stream URLs. Playback in libmpv is the next milestone.
          </p>
        {/if}
      </div>
    {:else}
      <div class="max-w-[1440px] mx-auto p-6 md:p-8">
        <div class="flex items-baseline gap-3 mb-4">
          <h2 class="text-xl font-semibold" style="font-family: var(--font-display);">
            Search results
          </h2>
          <span class="text-sm" style="color: var(--paper-500);">{results.length}</span>
        </div>

        {#if loading}
          <div class="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-6 gap-4">
            {#each Array(12) as _}
              <div class="space-y-2">
                <div class="aspect-[2/3] rounded-xl shimmer"></div>
                <div class="h-3 w-3/4 rounded shimmer"></div>
              </div>
            {/each}
          </div>
        {:else if error}
          <div
            class="max-w-xl rounded-2xl px-5 py-4 text-sm"
            style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
            role="alert"
          >
            {error}
            <button class="btn-primary ml-3 px-3 py-1 text-xs" onclick={() => void runSearch()}>
              Retry
            </button>
          </div>
        {:else}
          <div class="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-6 gap-4">
            {#each results as item (item.id)}
              <button
                class="poster-card group text-left rounded-xl overflow-hidden"
                style="background: var(--void-800); border: 1px solid var(--line);"
                onclick={() => void openShow(item)}
                aria-label="Open {item.title}"
              >
                <div class="aspect-[2/3] relative overflow-hidden" style="background: var(--void-700);">
                  {#if item.cover}
                    <img
                      src={item.cover}
                      alt=""
                      loading="lazy"
                      class="zoom w-full h-full object-cover"
                    />
                  {/if}
                </div>
                <div class="p-2.5">
                  <p class="text-[13px] font-semibold leading-snug line-clamp-2">{item.title}</p>
                  <p class="text-xs mt-1" style="color: var(--paper-500);">{item.year ?? ""}</p>
                </div>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </main>
</div>
