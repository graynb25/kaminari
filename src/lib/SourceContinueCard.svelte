<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { SourceContinueItem } from "./types";

  let {
    item,
    onOpen,
  }: {
    item: SourceContinueItem;
    onOpen: (item: SourceContinueItem) => void;
  } = $props();

  const percent = $derived(
    item.durationSec > 0
      ? Math.min(100, (item.positionSec / item.durationSec) * 100)
      : 0,
  );

  function formatTime(value: number): string {
    if (!Number.isFinite(value) || value <= 0) return "0:00";
    const minutes = Math.floor(value / 60);
    const seconds = Math.floor(value % 60);
    return `${minutes}:${String(seconds).padStart(2, "0")}`;
  }
</script>

<button
  class="library-source-card group block w-full text-left"
  aria-label="Resume {item.title}, episode {item.episodeNumber}"
  onclick={() => onOpen(item)}
>
  <div class="library-source-art">
    {#if item.cover}
      <img src={item.cover} alt="" loading="lazy" decoding="async" />
    {:else}
      <div
        class="w-full h-full"
        style="background: radial-gradient(circle at 30% 20%, rgb(139 138 255 / 0.2), transparent 60%), var(--void-800);"
      ></div>
    {/if}
    <span class="library-source-scrim" aria-hidden="true"></span>

    <span
      class="absolute top-2.5 left-2.5 inline-flex items-center gap-1.5 h-6 px-2 rounded-full text-[10px] font-semibold"
      style="background: rgb(8 8 15 / 0.72); border: 1px solid var(--line-strong); color: var(--volt-200); font-family: var(--font-mono); backdrop-filter: blur(10px);"
    >
      <span class="status-dot" style="width: 4px; height: 4px;" aria-hidden="true"></span>
      EP {item.episodeNumber}
    </span>
    <span
      class="absolute top-2.5 right-2.5 inline-flex items-center h-6 px-2 rounded-full text-[9px] font-semibold tracking-wider"
      style="background: rgb(8 8 15 / 0.62); border: 1px solid var(--line); color: var(--paper-300); backdrop-filter: blur(10px);"
    >
      RESUME
    </span>

    <span
      class="absolute right-3 bottom-3 h-9 w-9 grid place-items-center rounded-full"
      style="background: linear-gradient(180deg, var(--volt-400), var(--volt-500)); color: #fff; box-shadow: 0 6px 20px rgb(108 107 245 / 0.4);"
      aria-hidden="true"
    >
      <Icon name="play" size={14} />
    </span>
    <div class="absolute left-3 right-14 bottom-3">
      <p class="text-sm font-semibold leading-snug line-clamp-1">{item.title}</p>
      <p class="text-[10px] mt-0.5 truncate" style="color: var(--paper-300); font-family: var(--font-mono);">
        {item.extensionName} · {formatTime(item.positionSec)} / {formatTime(item.durationSec)}
      </p>
    </div>

    <span
      class="absolute left-0 right-0 bottom-0 h-[3px]"
      style="background: rgb(0 0 0 / 0.42);"
      aria-hidden="true"
    >
      <span
        class="block h-full"
        style="width: {percent}%; background: var(--volt-400); box-shadow: 0 0 10px var(--volt-400);"
      ></span>
    </span>
  </div>
</button>
