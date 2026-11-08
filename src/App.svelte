<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  const appWindow = getCurrentWindow();

  type MediaTitle = {
    romaji?: string | null;
    english?: string | null;
    native?: string | null;
  };

  type CoverImage = {
    extraLarge?: string | null;
    large?: string | null;
    color?: string | null;
  };

  type Media = {
    id: number;
    title: MediaTitle;
    coverImage?: CoverImage | null;
    bannerImage?: string | null;
    description?: string | null;
    genres?: string[] | null;
    averageScore?: number | null;
    seasonYear?: number | null;
    format?: string | null;
    status?: string | null;
    episodes?: number | null;
  };

  type Boot = {
    title: string;
    anilistEndpoint: string;
    limits: {
      extTimeoutSecs: number;
      extResolveTimeoutSecs: number;
      extMemoryBytes: number;
    };
  };

  type LibraryItem = {
    media: Media;
    status: string;
    addedAt: number;
  };

  let title = $state("Kaminari");
  let boot = $state<Boot | null>(null);
  let media = $state<Media[]>([]);
  let library = $state<LibraryItem[]>([]);
  let view = $state<"browse" | "library">("browse");
  let heading = $state("Trending now");
  let query = $state("");
  let lastQuery = $state("");
  let loading = $state(true);
  let error = $state<string | null>(null);
  let detail = $state<Media | null>(null);
  let detailLoading = $state(false);
  let playNote = $state<string | null>(null);

  function displayTitle(item: Media): string {
    return item.title.english || item.title.romaji || item.title.native || "Untitled";
  }

  function cover(item: Media): string | null {
    return item.coverImage?.extraLarge || item.coverImage?.large || null;
  }

  function clean(text?: string | null): string {
    if (!text) return "";
    return text
      .replace(/<br\s*\/?>/gi, " ")
      .replace(/<[^>]+>/g, "")
      .replace(/&quot;/g, '"')
      .replace(/&#039;/g, "'")
      .replace(/&amp;/g, "&")
      .replace(/\s+/g, " ")
      .trim();
  }

  function meta(item: Media): string {
    const bits: string[] = [];
    if (item.seasonYear) bits.push(String(item.seasonYear));
    if (item.format) bits.push(item.format);
    if (item.episodes) bits.push(`${item.episodes} eps`);
    return bits.join(" · ");
  }

  async function loadLibrary() {
    loading = true;
    error = null;
    view = "library";
    heading = "Your library";
    try {
      library = await invoke<LibraryItem[]>("library_list");
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function isInLibrary(id: number): boolean {
    return library.some((entry) => entry.media.id === id);
  }

  async function toggleLibrary(item: Media) {
    try {
      if (isInLibrary(item.id)) {
        await invoke("library_remove", { id: item.id });
      } else {
        await invoke("library_add", { media: item, status: "watching" });
      }
      library = await invoke<LibraryItem[]>("library_list");
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function closeDetail() {
    detail = null;
    playNote = null;
  }

  async function openDetail(item: Media) {
    detailLoading = true;
    error = null;
    playNote = null;
    try {
      detail = await invoke<Media>("anilist_detail", { id: item.id });
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      detailLoading = false;
    }
  }

  function playPlaceholder() {
    playNote =
      "Playback needs a source extension. AniList gives us the catalogue, not the stream. The extension host lands in P1.";
  }

  async function loadTrending() {
    loading = true;
    error = null;
    view = "browse";
    heading = "Trending now";
    lastQuery = "";
    try {
      media = await invoke<Media[]>("anilist_trending", { page: 1 });
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function runSearch() {
    const trimmed = query.trim();
    if (trimmed.length < 2) {
      await loadTrending();
      return;
    }
    loading = true;
    error = null;
    heading = `Results for "${trimmed}"`;
    lastQuery = trimmed;
    view = "browse";
    try {
      media = await invoke<Media[]>("anilist_search", { query: trimmed, page: 1 });
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    void (async () => {
      try {
        boot = await invoke<Boot>("boot_snapshot");
        title = boot.title;
      } catch {
        // Boot snapshot is informational. The shelves still load.
      }
      try {
        library = await invoke<LibraryItem[]>("library_list");
      } catch {
        // An empty library is a valid first run.
      }
      await loadTrending();
    })();
  });

  const hero = $derived(view === "browse" ? (media[0] ?? null) : null);
  const visibleMedia = $derived(view === "library" ? library.map((entry) => entry.media) : media);
  const visibleHeading = $derived(view === "library" ? "Your library" : heading);

  function retry() {
    if (view === "library") {
      void loadLibrary();
    } else if (lastQuery) {
      void runSearch();
    } else {
      void loadTrending();
    }
  }
</script>

<div class="h-screen flex flex-col">
  <header
    class="h-10 flex items-center justify-between px-3 shrink-0 select-none"
    data-tauri-drag-region
    style="background: var(--void-900); border-bottom: 1px solid var(--line);"
  >
    <div class="flex items-center gap-3" data-tauri-drag-region>
      <span class="font-semibold tracking-wide" data-tauri-drag-region>⚡ {title}</span>
      <nav class="flex gap-1 text-sm" style="color: var(--paper-300);">
        <button
          class="px-2 py-1 rounded-md"
          style="color: {view === 'library' ? 'var(--paper-50)' : 'var(--paper-300)'};"
          onclick={() => loadLibrary()}
        >
          Library
        </button>
        <button
          class="px-2 py-1 rounded-md"
          style="color: {view === 'browse' ? 'var(--paper-50)' : 'var(--paper-300)'};"
          onclick={() => loadTrending()}
        >
          Browse
        </button>
        <button class="px-2 py-1 rounded-md">Sources</button>
      </nav>
    </div>
    <form
      class="flex items-center gap-2"
      onsubmit={(event) => {
        event.preventDefault();
        void runSearch();
      }}
    >
      <input
        bind:value={query}
        placeholder="Search AniList"
        class="w-[300px] h-7 rounded-md px-3 text-sm outline-none"
        style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-50);"
      />
      <button
        type="submit"
        class="h-7 px-3 rounded-md text-sm font-semibold"
        style="background: var(--volt-500); color: var(--paper-50);"
      >
        Search
      </button>
    </form>
    <div class="flex items-center gap-1">
      <button
        class="window-control"
        aria-label="Minimize"
        title="Minimize"
        onclick={() => appWindow.minimize()}
      >
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M1 5h8" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
      <button
        class="window-control"
        aria-label="Maximize"
        title="Maximize"
        onclick={() => appWindow.toggleMaximize()}
      >
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <rect x="1.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
      <button
        class="window-control window-control-close"
        aria-label="Close"
        title="Close"
        onclick={() => appWindow.close()}
      >
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
    </div>
  </header>

  <main class="flex-1 overflow-y-auto">
    {#if detail}
      <div>
        <div class="relative h-[320px] overflow-hidden">
          {#if detail.bannerImage}
            <img
              src={detail.bannerImage}
              alt=""
              class="absolute inset-0 w-full h-full object-cover"
            />
          {:else if cover(detail)}
            <img
              src={cover(detail)}
              alt=""
              class="absolute inset-0 w-full h-full object-cover blur-2xl scale-110 opacity-60"
            />
          {/if}
          <div
            class="absolute inset-0"
            style="background: linear-gradient(90deg, var(--void-950) 5%, rgb(8 8 15 / 0.6) 55%, transparent 100%), linear-gradient(0deg, var(--void-950) 0%, transparent 60%);"
          ></div>
          <button
            class="absolute top-4 left-6 px-3 py-1.5 rounded-lg text-sm font-semibold"
            style="background: rgb(8 8 15 / 0.72); border: 1px solid var(--line); backdrop-filter: blur(12px);"
            onclick={closeDetail}
          >
            ← Back
          </button>
        </div>

        <div class="px-8 pb-12 -mt-28 relative">
          <div class="flex gap-6 items-end">
            <div
              class="w-[168px] shrink-0 aspect-[2/3] rounded-2xl overflow-hidden"
              style="background: {detail.coverImage?.color ?? "var(--void-700)"}; border: 1px solid var(--line); box-shadow: var(--shadow-card);"
            >
              {#if cover(detail)}
                <img src={cover(detail)} alt="" class="w-full h-full object-cover" />
              {/if}
            </div>
            <div class="pb-1 min-w-0">
              <h1 class="text-4xl font-bold leading-tight">{displayTitle(detail)}</h1>
              <p class="text-sm mt-2" style="color: var(--paper-300);">{meta(detail)}</p>
              <div class="flex flex-wrap gap-1.5 mt-3">
                {#each detail.genres ?? [] as genre}
                  <span
                    class="text-xs px-2 py-0.5 rounded-full"
                    style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
                  >
                    {genre}
                  </span>
                {/each}
              </div>
              <div class="flex gap-2 mt-5">
                <button
                  class="px-4 py-2 rounded-lg text-sm font-semibold"
                  style="background: var(--volt-500);"
                  onclick={playPlaceholder}
                >
                  ▶ Play
                </button>
                <button
                  class="px-4 py-2 rounded-lg text-sm font-semibold"
                  style="background: var(--void-800); border: 1px solid var(--line);"
                  onclick={() => toggleLibrary(detail as Media)}
                >
                  {isInLibrary(detail.id) ? "✓ In Library" : "+ Library"}
                </button>
              </div>
            </div>
          </div>

          {#if playNote}
            <div
              class="mt-6 max-w-2xl p-3 rounded-lg text-sm"
              style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
            >
              {playNote}
            </div>
          {/if}

          <div class="mt-8 grid grid-cols-1 xl:grid-cols-3 gap-8">
            <div class="xl:col-span-2">
              <h2 class="text-lg font-semibold mb-2">Synopsis</h2>
              <p class="text-sm leading-relaxed" style="color: var(--paper-300);">
                {clean(detail.description) || "AniList has no synopsis for this title yet."}
              </p>

              <h2 class="text-lg font-semibold mt-8 mb-3">Episodes</h2>
              {#if detail.episodes && detail.episodes > 0}
                <div class="grid grid-cols-3 sm:grid-cols-5 md:grid-cols-7 gap-2">
                  {#each Array(detail.episodes) as _, index}
                    <button
                      class="h-10 rounded-lg text-sm font-semibold"
                      style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
                      onclick={playPlaceholder}
                    >
                      {index + 1}
                    </button>
                  {/each}
                </div>
              {:else}
                <p class="text-sm" style="color: var(--paper-500);">
                  AniList has no episode count for this title.
                </p>
              {/if}
            </div>

            <aside
              class="self-start p-4 rounded-2xl space-y-3"
              style="background: var(--void-900); border: 1px solid var(--line);"
            >
              <h2 class="text-sm font-semibold uppercase tracking-wider" style="color: var(--paper-500);">
                Details
              </h2>
              <div class="flex justify-between text-sm">
                <span style="color: var(--paper-500);">Score</span>
                <span class="font-semibold">{detail.averageScore ?? "n/a"}</span>
              </div>
              <div class="flex justify-between text-sm">
                <span style="color: var(--paper-500);">Format</span>
                <span>{detail.format ?? "n/a"}</span>
              </div>
              <div class="flex justify-between text-sm">
                <span style="color: var(--paper-500);">Status</span>
                <span>{detail.status ?? "n/a"}</span>
              </div>
              <div class="flex justify-between text-sm">
                <span style="color: var(--paper-500);">Episodes</span>
                <span>{detail.episodes ?? "n/a"}</span>
              </div>
              <div class="flex justify-between text-sm">
                <span style="color: var(--paper-500);">Year</span>
                <span>{detail.seasonYear ?? "n/a"}</span>
              </div>
            </aside>
          </div>
        </div>
      </div>
    {:else if loading}
      <div class="p-6 space-y-6">
        <div class="h-64 rounded-2xl animate-pulse" style="background: var(--void-800);"></div>
        <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
          {#each Array(12) as _}
            <div class="space-y-2">
              <div class="aspect-[2/3] rounded-2xl animate-pulse" style="background: var(--void-800);"></div>
              <div class="h-3 w-3/4 rounded animate-pulse" style="background: var(--void-800);"></div>
              <div class="h-2 w-1/2 rounded animate-pulse" style="background: var(--void-800);"></div>
            </div>
          {/each}
        </div>
      </div>
    {:else if error}
      <div class="p-6">
        <div
          class="max-w-xl p-5 rounded-2xl"
          style="background: var(--void-800); border: 1px solid var(--line);"
        >
          <h2 class="text-lg font-semibold">AniList did not answer</h2>
          <p class="text-sm mt-2" style="color: var(--paper-300);">{error}</p>
          <p class="text-xs mt-2" style="color: var(--paper-500);">
            Check your connection. A rate limit clears itself in under a minute.
          </p>
          <button
            class="mt-4 px-3 py-1.5 rounded-md text-sm font-semibold"
            style="background: var(--volt-500);"
            onclick={retry}
          >
            Retry
          </button>
        </div>
      </div>
    {:else if visibleMedia.length === 0}
      <div class="h-full grid place-items-center p-6">
        <div class="text-center max-w-md">
          <p class="text-4xl mb-3">⚡</p>
          <h2 class="text-xl font-semibold">
            {view === "library" ? "Your library is empty" : "Nothing here yet"}
          </h2>
          <p class="text-sm mt-2" style="color: var(--paper-300);">
            {view === "library"
              ? "Add a title from Browse and it will live here, with progress and history beside it."
              : lastQuery
                ? `No results for "${lastQuery}". Try romaji or a shorter title.`
                : "AniList returned an empty shelf. Try again in a moment."}
          </p>
          <button
            class="mt-4 px-4 py-2 rounded-lg text-sm font-semibold"
            style="background: var(--volt-500);"
            onclick={() => (view === "library" ? loadTrending() : retry())}
          >
            {view === "library" ? "Browse trending" : "Reload"}
          </button>
        </div>
      </div>
    {:else}
      {#if hero}
        <section class="relative h-[340px] overflow-hidden">
          {#if hero.bannerImage}
            <img
              src={hero.bannerImage}
              alt=""
              class="absolute inset-0 w-full h-full object-cover"
            />
          {:else if cover(hero)}
            <img
              src={cover(hero)}
              alt=""
              class="absolute inset-0 w-full h-full object-cover blur-2xl scale-110 opacity-60"
            />
          {/if}
          <div
            class="absolute inset-0"
            style="background: linear-gradient(90deg, var(--void-950) 10%, rgb(8 8 15 / 0.65) 55%, transparent 100%), linear-gradient(0deg, var(--void-950) 0%, transparent 55%);"
          ></div>
          <div class="relative h-full flex items-end p-8 max-w-3xl">
            <div>
              <p class="text-xs font-semibold tracking-[0.18em]" style="color: var(--volt-300);">
                TOP PICK {hero.averageScore ? `· ${hero.averageScore}%` : ""}
              </p>
              <h1 class="text-4xl font-bold mt-2 leading-tight">{displayTitle(hero)}</h1>
              <p class="text-sm mt-2" style="color: var(--paper-300);">
                {meta(hero)}
              </p>
              <p class="text-sm mt-3 max-w-xl line-clamp-2" style="color: var(--paper-300);">
                {clean(hero.description)}
              </p>
              <div class="flex gap-2 mt-5">
                <button
                  class="px-4 py-2 rounded-lg text-sm font-semibold"
                  style="background: var(--volt-500);"
                >
                  ▶ Play
                </button>
                <button
                  class="px-4 py-2 rounded-lg text-sm font-semibold"
                  style="background: var(--void-800); border: 1px solid var(--line);"
                  onclick={() => toggleLibrary(hero)}
                >
                  {isInLibrary(hero.id) ? "✓ In Library" : "+ Library"}
                </button>
              </div>
            </div>
          </div>
        </section>
      {/if}

      <section class="p-6">
        <div class="flex items-baseline gap-3 mb-4">
          <h2 class="text-lg font-semibold">{visibleHeading}</h2>
          <span class="text-sm" style="color: var(--paper-500);">{visibleMedia.length} titles</span>
        </div>
        <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
          {#each visibleMedia as item (item.id)}
            <article
              class="poster-card group relative rounded-2xl overflow-hidden cursor-pointer"
              style="background: var(--void-800); border: 1px solid var(--line);"
            >
              <button class="block w-full text-left" onclick={() => openDetail(item)}>
                <div
                  class="aspect-[2/3] relative"
                  style="background: {item.coverImage?.color ?? "var(--void-700)"};"
                >
                  {#if cover(item)}
                    <img
                      src={cover(item)}
                      alt=""
                      loading="lazy"
                      class="w-full h-full object-cover"
                    />
                  {/if}
                  {#if item.averageScore}
                    <span
                      class="absolute top-2 right-2 text-xs font-semibold px-1.5 py-0.5 rounded"
                      style="background: rgb(8 8 15 / 0.78); color: var(--volt-300);"
                    >
                      ★ {item.averageScore}
                    </span>
                  {/if}
                </div>
                <div class="p-2.5">
                  <p class="text-sm font-semibold leading-snug line-clamp-2">{displayTitle(item)}</p>
                  <p class="text-xs mt-1" style="color: var(--paper-500);">{meta(item)}</p>
                </div>
              </button>
              <button
                class="absolute bottom-2 right-2 h-7 w-7 grid place-items-center rounded-full text-sm font-bold opacity-0 group-hover:opacity-100 focus-visible:opacity-100 transition-opacity duration-150"
                style="background: rgb(8 8 15 / 0.86); border: 1px solid var(--line); color: var(--paper-50);"
                aria-label={isInLibrary(item.id) ? `Remove ${displayTitle(item)} from library` : `Add ${displayTitle(item)} to library`}
                onclick={(event) => {
                  event.stopPropagation();
                  void toggleLibrary(item);
                }}
              >
                {isInLibrary(item.id) ? "✓" : "+"}
              </button>
            </article>
          {/each}
        </div>
      </section>
    {/if}
  </main>
</div>
