<script lang="ts">
  import Icon from "./Icon.svelte";
  import { coverOf, displayTitle, metaOf, scoreOutOfTen } from "./format";
  import type { Media } from "./types";

  let {
    item,
    inLibrary,
    progress = null,
    badge = null,
    onOpen,
    onToggleLibrary,
    onPlay,
  }: {
    item: Media;
    inLibrary: boolean;
    progress?: { watched: number; total: number } | null;
    badge?: string | null;
    onOpen: (item: Media) => void;
    onToggleLibrary: (item: Media) => void;
    onPlay: (item: Media) => void;
  } = $props();

  const title = $derived(displayTitle(item));
  const art = $derived(coverOf(item));
  const isAiring = $derived(item.status === "RELEASING");
  const isCompleted = $derived(item.status === "FINISHED");
  const percent = $derived(
    progress && progress.total > 0
      ? Math.min(100, Math.max(0, (progress.watched / progress.total) * 100))
      : 0,
  );

  // Card body and quick actions are siblings. Nothing interactive nests.
  // Enter, P and L work from either button.
  function onKey(event: KeyboardEvent) {
    const key = event.key.toLowerCase();
    if (key === "enter") {
      event.preventDefault();
      onOpen(item);
    } else if (key === "p") {
      event.preventDefault();
      onPlay(item);
    } else if (key === "l") {
      event.preventDefault();
      onToggleLibrary(item);
    }
  }
</script>

<article
  class="poster-card group"
  aria-label="{title}, {metaOf(item)}"
>
  <div class="poster-art aspect-[2/3]">
    <button
      class="absolute inset-0 w-full h-full text-left"
      onclick={() => onOpen(item)}
      onkeydown={onKey}
      aria-label="Open {title}"
    >
      {#if art}
        <img src={art} alt="" loading="lazy" decoding="async" class="zoom w-full h-full object-cover" />
      {:else}
        <span
          class="w-full h-full grid place-items-center"
          style="background: radial-gradient(circle at 30% 20%, rgb(139 138 255 / 0.18), transparent 60%), var(--void-800);"
        >
          <span class="text-4xl font-bold" style="color: var(--paper-500); font-family: var(--font-display);">
            {title.slice(0, 1)}
          </span>
        </span>
      {/if}
      <span class="poster-scrim" aria-hidden="true"></span>
      {#if badge}
        <span class="poster-badge">{badge}</span>
      {:else if isAiring}
        <span class="poster-badge is-airing">
          <span class="status-dot" style="width: 4px; height: 4px;" aria-hidden="true"></span>
          AIRING
        </span>
      {:else if isCompleted}
        <span class="poster-badge is-completed" title="Finished airing">
          <Icon name="check" size={9} />
          COMPLETED
        </span>
      {/if}
      {#if item.averageScore}
        <span
          class="poster-score"
          title="AniList score {item.averageScore} out of 100"
        >
          <Icon name="star" size={9} />
          {scoreOutOfTen(item.averageScore)}
        </span>
      {:else}
        <span class="poster-score is-unrated" title="AniList has no score yet">NR</span>
      {/if}
      {#if progress && progress.total > 0}
        <span class="poster-progress" aria-hidden="true">
          <span style="width: {percent}%;"></span>
        </span>
      {/if}
    </button>

    <div class="poster-quick">
      <button
        class="quick-play"
        onclick={() => onPlay(item)}
        onkeydown={onKey}
        aria-label="Play {title}"
      >
        <Icon name="play" size={11} />
        Play
      </button>
      <button
        class="quick-save"
        onclick={() => onToggleLibrary(item)}
        onkeydown={onKey}
        aria-label={inLibrary ? `Remove ${title} from library` : `Add ${title} to library`}
      >
        <Icon name={inLibrary ? "check" : "plus"} size={11} />
        {inLibrary ? "Saved" : "Save"}
      </button>
    </div>
  </div>

  <button class="poster-body" onclick={() => onOpen(item)} onkeydown={onKey}>
    <span class="poster-title">{title}</span>
    <span class="poster-meta">{metaOf(item)}</span>
  </button>
</article>
