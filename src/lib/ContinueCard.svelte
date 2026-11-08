<script lang="ts">
  import { coverOf, displayTitle } from "./format";
  import type { ContinueItem, Media } from "./types";

  let { item, onOpen }: { item: ContinueItem; onOpen: (media: Media) => void } = $props();

  const art = $derived(item.media.bannerImage || coverOf(item.media));
  const percent = $derived(
    item.totalEpisodes > 0 ? Math.min(100, (item.watchedCount / item.totalEpisodes) * 100) : 0,
  );
</script>

<button
  class="poster-card group block w-full text-left rounded-2xl overflow-hidden"
  style="background: var(--void-800); border: 1px solid var(--line);"
  aria-label="Resume {displayTitle(item.media)}, episode {item.episode}"
  onclick={() => onOpen(item.media)}
>
  <div class="relative aspect-video overflow-hidden" style="background: var(--void-700);">
    {#if art}
      <img src={art} alt="" loading="lazy" class="zoom w-full h-full object-cover" />
    {/if}
    <div
      class="absolute inset-0"
      style="background: linear-gradient(0deg, rgb(8 8 15 / 0.92) 0%, rgb(8 8 15 / 0.25) 55%, transparent 100%);"
      aria-hidden="true"
    ></div>
    <span
      class="absolute top-2 left-2 text-[11px] font-semibold px-2 py-0.5 rounded-full"
      style="background: rgb(8 8 15 / 0.78); color: var(--volt-300); font-family: var(--font-mono);"
    >
      EP {item.episode}
    </span>
    <span
      class="absolute right-2 bottom-2 h-9 w-9 grid place-items-center rounded-full"
      style="background: var(--volt-500); color: var(--paper-50); box-shadow: var(--shadow-card);"
      aria-hidden="true"
    >
      ▶
    </span>
    <div class="absolute left-3 right-14 bottom-3">
      <p class="text-sm font-semibold leading-snug line-clamp-1">{displayTitle(item.media)}</p>
      <p class="text-[11px] mt-0.5" style="color: var(--paper-300); font-family: var(--font-mono);">
        {item.watchedCount} of {item.totalEpisodes || "?"} watched
      </p>
    </div>
  </div>
  <div class="h-1" style="background: var(--void-700);" aria-hidden="true">
    <div class="h-full" style="width: {percent}%; background: var(--volt-400);"></div>
  </div>
</button>
