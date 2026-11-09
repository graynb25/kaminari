<script lang="ts">
  import { onMount, tick } from "svelte";
  import PosterCard from "./PosterCard.svelte";
  import type { LibraryItem, Media } from "./types";

  const count = 500;
  const cover =
    "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='300' height='450'%3E%3Cdefs%3E%3ClinearGradient id='g' x1='0' y1='0' x2='1' y2='1'%3E%3Cstop offset='0' stop-color='%232b2b46'/%3E%3Cstop offset='1' stop-color='%23121220'/%3E%3C/linearGradient%3E%3C/defs%3E%3Crect width='300' height='450' fill='url(%23g)'/%3E%3Ccircle cx='90' cy='110' r='70' fill='%236c6bf5' opacity='0.28'/%3E%3Ctext x='150' y='250' fill='%23b7b5ff' font-family='sans-serif' font-size='54' font-weight='700' text-anchor='middle'%3EK%3C/text%3E%3C/svg%3E";
  const items: LibraryItem[] = Array.from({ length: count }, (_, index) => {
    const media: Media = {
      id: 900000 + index,
      title: {
        english: `Perf Title ${String(index + 1).padStart(3, "0")}`,
        romaji: null,
        native: null,
      },
      coverImage: { large: cover, color: "#23233a" },
      bannerImage: null,
      description: null,
      genres: ["Action"],
      averageScore: 60 + (index % 40),
      seasonYear: 2026,
      format: index % 3 === 0 ? "TV" : "ONA",
      status: index % 2 === 0 ? "RELEASING" : "FINISHED",
      episodes: 12,
    };
    return {
      media,
      status: "watching",
      addedAt: Date.now() - index * 1000,
      watchedCount: index % 13,
    };
  });

  let mountMs = 0;

  onMount(async () => {
    const started = (window as unknown as { __perfStart?: number }).__perfStart ?? performance.now();
    await tick();
    mountMs = performance.now() - started;
    (window as unknown as { __perfMountMs?: number }).__perfMountMs = mountMs;
    document.body.dataset.perfReady = "true";
  });

  function noop() {}
</script>

<div class="h-screen flex flex-col" style="background: var(--page-bg);">
  <header class="shrink-0 flex flex-wrap items-baseline gap-3 px-5 py-4 border-b" style="border-color: var(--line);">
    <h1 class="page-title text-2xl">Perf harness</h1>
    <p class="text-sm" style="color: var(--text-3);">{count} library items, no network images</p>
    <p class="text-xs mono ml-auto" style="color: var(--text-4);">mount {mountMs.toFixed(1)} ms</p>
  </header>
  <div id="perf-scroll" class="flex-1 overflow-y-auto" data-perf-scroll>
    <div class="stagger-grid grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6 gap-5 p-5">
      {#each items as entry (entry.media.id)}
        <div data-perf-card>
          <PosterCard
            item={entry.media}
            inLibrary={true}
            progress={{ watched: entry.watchedCount, total: 12 }}
            onOpen={noop}
            onToggleLibrary={noop}
            onPlay={noop}
          />
        </div>
      {/each}
    </div>
  </div>
</div>
