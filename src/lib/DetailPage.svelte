<script lang="ts">
  import { coverOf, cleanHtml, displayTitle, metaOf } from "./format";
  import type { Media } from "./types";

  let {
    media,
    inLibrary,
    playNote,
    onBack,
    onToggleLibrary,
    onPlay,
  }: {
    media: Media;
    inLibrary: boolean;
    playNote: string | null;
    onBack: () => void;
    onToggleLibrary: (item: Media) => void;
    onPlay: (item: Media) => void;
  } = $props();

  const art = $derived(coverOf(media));
  const episodeCount = $derived(media.episodes && media.episodes > 0 ? Math.min(media.episodes, 200) : 0);
</script>

<div>
  <div class="relative h-[320px] overflow-hidden">
    {#if media.bannerImage}
      <img src={media.bannerImage} alt="" class="absolute inset-0 w-full h-full object-cover" />
    {:else if art}
      <img src={art} alt="" class="absolute inset-0 w-full h-full object-cover blur-2xl scale-110 opacity-60" />
    {:else}
      <div class="absolute inset-0" style="background: var(--void-800);"></div>
    {/if}
    <div
      class="absolute inset-0"
      style="background: linear-gradient(90deg, var(--void-950) 5%, rgb(8 8 15 / 0.6) 55%, transparent 100%), linear-gradient(0deg, var(--void-950) 0%, transparent 60%);"
      aria-hidden="true"
    ></div>
    <button
      class="btn-ghost absolute top-4 left-6 px-3 py-1.5 text-sm"
      style="background: rgb(8 8 15 / 0.72); backdrop-filter: blur(12px);"
      onclick={onBack}
    >
      ← Back
    </button>
  </div>

  <div class="px-6 md:px-8 pb-12 -mt-28 relative max-w-[1440px] mx-auto">
    <div class="flex gap-6 items-end">
      <div
        class="w-[168px] md:w-[224px] shrink-0 aspect-[2/3] rounded-2xl overflow-hidden"
        style="background: {media.coverImage?.color ?? 'var(--void-700)'}; border: 1px solid var(--line); box-shadow: var(--shadow-card);"
      >
        {#if art}
          <img src={art} alt="" class="w-full h-full object-cover" />
        {:else}
          <div class="w-full h-full grid place-items-center">
            <span class="text-5xl font-bold" style="color: var(--paper-500); font-family: var(--font-display);">
              {displayTitle(media).slice(0, 1)}
            </span>
          </div>
        {/if}
      </div>
      <div class="pb-1 min-w-0">
        <p class="eyebrow">{media.format ?? "Title"} {media.averageScore ? `· ${media.averageScore}%` : ""}</p>
        <h1
          class="mt-1 leading-tight"
          style="font-family: var(--font-display); font-size: 32px; font-weight: 700; letter-spacing: -0.01em;"
        >
          {displayTitle(media)}
        </h1>
        {#if media.title.english && media.title.romaji && media.title.english !== media.title.romaji}
          <p class="text-sm mt-1" style="color: var(--paper-500); font-family: var(--font-jp);">
            {media.title.romaji}
          </p>
        {/if}
        <p class="text-sm mt-2" style="color: var(--paper-300);">{metaOf(media)}</p>
        <div class="flex flex-wrap gap-1.5 mt-3">
          {#each media.genres ?? [] as genre}
            <span class="chip text-xs px-2 py-0.5">{genre}</span>
          {/each}
        </div>
        <div class="flex flex-wrap gap-2 mt-5">
          <button class="btn-primary px-4 py-2 text-sm" onclick={() => onPlay(media)}>
            ▶ Play
          </button>
          <button class="btn-ghost px-4 py-2 text-sm" onclick={() => onToggleLibrary(media)}>
            {inLibrary ? "✓ In Library" : "+ Library"}
          </button>
        </div>
      </div>
    </div>

    {#if playNote}
      <div
        class="mt-6 max-w-2xl p-3 rounded-xl text-sm leading-relaxed"
        style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
        role="status"
      >
        {playNote}
      </div>
    {/if}

    <div class="mt-8 grid grid-cols-1 xl:grid-cols-3 gap-8">
      <div class="xl:col-span-2 min-w-0">
        <h2 class="text-lg font-semibold mb-2">Synopsis</h2>
        <p class="text-sm leading-relaxed" style="color: var(--paper-300);">
          {cleanHtml(media.description) || "AniList has no synopsis for this title yet."}
        </p>

        <h2 class="text-lg font-semibold mt-8 mb-3">Episodes</h2>
        {#if episodeCount > 0}
          <ol class="grid grid-cols-4 sm:grid-cols-6 md:grid-cols-8 gap-2" aria-label="Episodes">
            {#each Array(episodeCount) as _, index}
              <li>
                <button
                  class="w-full h-10 rounded-lg text-sm font-semibold"
                  style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300); font-family: var(--font-mono);"
                  onclick={() => onPlay(media)}
                  aria-label="Play episode {index + 1}"
                >
                  {index + 1}
                </button>
              </li>
            {/each}
          </ol>
          {#if (media.episodes ?? 0) > 200}
            <p class="text-xs mt-2" style="color: var(--paper-500);">
              Showing the first 200 of {media.episodes} listed episodes.
            </p>
          {/if}
        {:else}
          <p class="text-sm" style="color: var(--paper-500);">
            AniList has no episode count for this title.
          </p>
        {/if}
      </div>

      <aside
        class="self-start p-4 rounded-2xl space-y-3"
        style="background: var(--void-900); border: 1px solid var(--line);"
        aria-label="Details"
      >
        <h2 class="text-[11px] font-semibold uppercase tracking-wider" style="color: var(--paper-500);">
          Details
        </h2>
        <div class="flex justify-between text-sm">
          <span style="color: var(--paper-500);">Score</span>
          <span class="font-semibold" style="font-family: var(--font-mono);">{media.averageScore ?? "n/a"}</span>
        </div>
        <div class="flex justify-between text-sm">
          <span style="color: var(--paper-500);">Format</span>
          <span>{media.format ?? "n/a"}</span>
        </div>
        <div class="flex justify-between text-sm">
          <span style="color: var(--paper-500);">Status</span>
          <span>{media.status ?? "n/a"}</span>
        </div>
        <div class="flex justify-between text-sm">
          <span style="color: var(--paper-500);">Episodes</span>
          <span style="font-family: var(--font-mono);">{media.episodes ?? "n/a"}</span>
        </div>
        <div class="flex justify-between text-sm">
          <span style="color: var(--paper-500);">Year</span>
          <span style="font-family: var(--font-mono);">{media.seasonYear ?? "n/a"}</span>
        </div>
        <div class="flex justify-between text-sm">
          <span style="color: var(--paper-500);">AniList id</span>
          <span style="font-family: var(--font-mono);">{media.id}</span>
        </div>
      </aside>
    </div>
  </div>
</div>
