<script lang="ts">
  import Icon from "./Icon.svelte";
  import { coverOf, displayTitle } from "./format";
  import type { ContinueItem, Media } from "./types";

  let { item, onOpen }: { item: ContinueItem; onOpen: (media: Media) => void } = $props();

  const art = $derived(item.media.bannerImage || coverOf(item.media));
  const percent = $derived(
    item.totalEpisodes > 0 ? Math.min(100, (item.watchedCount / item.totalEpisodes) * 100) : 0,
  );
</script>

<button
  class="library-source-card group block w-full text-left"
  aria-label="Resume {displayTitle(item.media)}, episode {item.episode}"
  onclick={() => onOpen(item.media)}
>
  <div class="library-source-art">
    {#if art}
      <img src={art} alt="" loading="lazy" decoding="async" />
    {:else}
      <div class="w-full h-full" style="background: radial-gradient(circle at 30% 20%, rgb(139 138 255 / 0.2), transparent 60%), var(--void-800);"></div>
    {/if}
    <span class="library-source-scrim" aria-hidden="true"></span>

    <span class="absolute top-2.5 left-2.5 inline-flex items-center gap-1.5 h-6 px-2 rounded-full text-[10px] font-semibold" style="background: rgb(8 8 15 / 0.72); border: 1px solid var(--line-strong); color: var(--volt-200); font-family: var(--font-mono); backdrop-filter: blur(10px);">
      <span class="status-dot" style="width: 4px; height: 4px;" aria-hidden="true"></span>
      EP {item.episode}
    </span>
    <span class="absolute top-2.5 right-2.5 inline-flex items-center h-6 px-2 rounded-full text-[9px] font-semibold tracking-wider" style="background: rgb(8 8 15 / 0.62); border: 1px solid var(--line); color: var(--paper-300); backdrop-filter: blur(10px);">
      RESUME
    </span>

    <span class="absolute right-3 bottom-3 h-9 w-9 grid place-items-center rounded-full" style="background: linear-gradient(180deg, var(--volt-400), var(--volt-500)); color: #fff; box-shadow: 0 6px 20px rgb(108 107 245 / 0.4);" aria-hidden="true">
      <Icon name="play" size={14} />
    </span>
    <div class="absolute left-3 right-14 bottom-3">
      <p class="text-sm font-semibold leading-snug line-clamp-1">{displayTitle(item.media)}</p>
      <p class="text-[10px] mt-0.5" style="color: var(--paper-300); font-family: var(--font-mono);">
        {item.watchedCount} of {item.totalEpisodes || "?"} watched
      </p>
    </div>

    <span class="absolute left-0 right-0 bottom-0 h-[3px]" style="background: rgb(0 0 0 / 0.42);" aria-hidden="true">
      <span class="block h-full" style="width: {percent}%; background: var(--volt-400); box-shadow: 0 0 10px var(--volt-400);"></span>
    </span>
  </div>
</button>
