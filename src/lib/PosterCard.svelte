<script lang="ts">
  import { coverOf, displayTitle, metaOf, scoreOutOfTen } from "./format";
  import type { Media } from "./types";

  let {
    item,
    inLibrary,
    onOpen,
    onToggleLibrary,
    onPlay,
  }: {
    item: Media;
    inLibrary: boolean;
    onOpen: (item: Media) => void;
    onToggleLibrary: (item: Media) => void;
    onPlay: (item: Media) => void;
  } = $props();

  const title = $derived(displayTitle(item));
  const art = $derived(coverOf(item));

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
  class="poster-card group relative rounded-2xl overflow-hidden"
  style="background: var(--void-800); border: 1px solid var(--line);"
  aria-label="{title}, {metaOf(item)}"
>
  <div class="aspect-[2/3] relative overflow-hidden" style="background: {item.coverImage?.color ?? 'var(--void-700)'};">
    <button
      class="absolute inset-0 w-full h-full text-left"
      onclick={() => onOpen(item)}
      onkeydown={onKey}
      aria-label="Open {title}"
    >
      {#if art}
        <img src={art} alt="" loading="lazy" class="zoom w-full h-full object-cover" />
      {:else}
        <span class="w-full h-full grid place-items-center">
          <span class="text-4xl font-bold" style="color: var(--paper-500); font-family: var(--font-display);">
            {title.slice(0, 1)}
          </span>
        </span>
      {/if}
      <span
        class="absolute inset-x-0 bottom-0 h-16 pointer-events-none"
        style="background: linear-gradient(0deg, rgb(8 8 15 / 0.75), transparent);"
        aria-hidden="true"
      ></span>
      {#if item.averageScore}
        <span
          class="absolute top-2 right-2 text-xs font-semibold px-1.5 py-0.5 rounded-md"
          style="background: rgb(8 8 15 / 0.78); color: var(--volt-300); font-family: var(--font-mono);"
          title="AniList score {item.averageScore} out of 100"
        >
          ★ {scoreOutOfTen(item.averageScore)}
        </span>
      {/if}
    </button>
    <div class="quick absolute inset-x-2 bottom-2 flex gap-1.5">
      <button
        class="flex-1 text-xs font-semibold px-2 py-1.5 rounded-lg"
        style="background: var(--volt-500); color: var(--paper-50);"
        onclick={() => onPlay(item)}
        onkeydown={onKey}
        aria-label="Play {title}"
      >
        ▶ Play
      </button>
      <button
        class="flex-1 text-xs font-semibold px-2 py-1.5 rounded-lg"
        style="background: rgb(8 8 15 / 0.86); border: 1px solid var(--line); color: var(--paper-50);"
        onclick={() => onToggleLibrary(item)}
        onkeydown={onKey}
        aria-label={inLibrary ? `Remove ${title} from library` : `Add ${title} to library`}
      >
        {inLibrary ? "✓ Saved" : "+ Save"}
      </button>
    </div>
  </div>
  <button class="block w-full text-left p-2.5" onclick={() => onOpen(item)} onkeydown={onKey}>
    <p class="text-[13px] font-semibold leading-snug line-clamp-2">{title}</p>
    <p class="text-xs mt-1" style="color: var(--paper-500);">{metaOf(item)}</p>
  </button>
</article>
