<script lang="ts">
  import Artwork from "./Artwork.svelte";
  import Icon from "./Icon.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import { coverOf, cleanHtml, displayTitle, metaOf, scoreLabel, scoreOutOfTen } from "./format";
  import { countdownLabel, formatScheduleDate, formatScheduleTime } from "./countdown";
  import type { Media } from "./types";

  let {
    media,
    inLibrary,
    libraryIds,
    status,
    watched,
    onBack,
    onToggleLibrary,
    onSetStatus,
    onPlay,
    onToggleWatched,
    onMarkAll,
    onOpenRecommendation,
  }: {
    media: Media;
    inLibrary: boolean;
    libraryIds: Set<number>;
    status: string | null;
    watched: number[];
    onBack: () => void;
    onToggleLibrary: (item: Media) => void;
    onSetStatus: (item: Media, status: string) => void;
    onPlay: (item: Media) => void;
    onToggleWatched: (episode: number) => void;
    onMarkAll: (watched: boolean) => void;
    onOpenRecommendation: (item: Media) => void;
  } = $props();

  const art = $derived(coverOf(media));
  const recommendations = $derived(
    (media.recommendations?.nodes ?? [])
      .map((node) => node.mediaRecommendation)
      .filter((item): item is Media => Boolean(item && item.id !== media.id))
      .slice(0, 12),
  );
  const seasonOrder = ["SEQUEL", "PREQUEL", "SIDE_STORY", "SPIN_OFF", "ALTERNATIVE"];
  // AniList relation nodes can be manga, so keep anime formats only.
  const animeFormats = ["TV", "TV_SHORT", "MOVIE", "SPECIAL", "OVA", "ONA", "MUSIC"];
  const seasonLabels: Record<string, string> = {
    SEQUEL: "Sequel",
    PREQUEL: "Prequel",
    SIDE_STORY: "Side story",
    SPIN_OFF: "Spin-off",
    ALTERNATIVE: "Alternative",
  };
  const relatedSeasons = $derived.by(() => {
    const edges = media.relations?.edges ?? [];
    const out: { item: Media; relation: string }[] = [];
    for (const edge of edges) {
      const node = edge.node;
      if (!node || node.id === media.id) continue;
      if (!seasonOrder.includes(edge.relationType)) continue;
      if (!node.format || !animeFormats.includes(node.format)) continue;
      out.push({ item: node, relation: edge.relationType });
    }
    out.sort((a, b) => {
      const order = seasonOrder.indexOf(a.relation) - seasonOrder.indexOf(b.relation);
      if (order !== 0) return order;
      return (a.item.seasonYear ?? 0) - (b.item.seasonYear ?? 0);
    });
    return out;
  });
  const episodeCount = $derived(media.episodes && media.episodes > 0 ? Math.min(media.episodes, 200) : 0);
  const watchedCount = $derived(watched.length);
  const percent = $derived(episodeCount > 0 ? Math.round((watchedCount / episodeCount) * 100) : 0);
  let now = $state(Math.floor(Date.now() / 1000));
  const nextAiring = $derived(media.nextAiringEpisode ?? null);
  const nextLabel = $derived(nextAiring ? countdownLabel(nextAiring.airingAt, now) : null);
  let menuOpen = $state(false);
  let menuEl: HTMLDetailsElement | null = $state(null);

  const statuses = [
    { id: "watching", label: "Watching" },
    { id: "planning", label: "Planning" },
    { id: "completed", label: "Completed" },
    { id: "paused", label: "Paused" },
    { id: "dropped", label: "Dropped" },
  ];

  function statusLabel(value: string | null): string {
    return statuses.find((entry) => entry.id === value)?.label ?? "Library";
  }

  function isWatched(episode: number): boolean {
    return watched.includes(episode);
  }

  function chooseStatus(next: string) {
    menuOpen = false;
    onSetStatus(media, next);
  }

  function onMenuKey(event: KeyboardEvent) {
    if (event.key === "Escape" && menuOpen) {
      event.preventDefault();
      event.stopPropagation();
      menuOpen = false;
    }
  }

  $effect(() => {
    const id = window.setInterval(() => {
      if (document.visibilityState === "visible") now = Math.floor(Date.now() / 1000);
    }, 1000);
    const onVis = () => {
      if (document.visibilityState === "visible") now = Math.floor(Date.now() / 1000);
    };
    document.addEventListener("visibilitychange", onVis);
    return () => {
      window.clearInterval(id);
      document.removeEventListener("visibilitychange", onVis);
    };
  });

  $effect(() => {
    if (!menuOpen || !menuEl) return;
    const onPointer = (event: PointerEvent) => {
      if (menuEl && !menuEl.contains(event.target as Node)) menuOpen = false;
    };
    document.addEventListener("pointerdown", onPointer);
    return () => document.removeEventListener("pointerdown", onPointer);
  });
</script>

<div>
  <div class="relative h-[320px] overflow-hidden">
    {#if media.bannerImage}
      <Artwork src={media.bannerImage} alt="" loading="eager" fill>
        {#snippet fallback()}
          {#if art}
            <div class="absolute inset-0 overflow-hidden blur-2xl scale-110 opacity-50" aria-hidden="true">
              <Artwork src={art} alt="" loading="eager">
                {#snippet fallback()}
                  <div class="absolute inset-0" style="background: radial-gradient(circle at 30% 10%, var(--accent-wash-strong), transparent 60%), var(--void-800);"></div>
                {/snippet}
              </Artwork>
            </div>
          {:else}
            <div class="absolute inset-0" style="background: radial-gradient(circle at 30% 10%, var(--accent-wash-strong), transparent 60%), var(--void-800);"></div>
          {/if}
        {/snippet}
      </Artwork>
    {:else if art}
      <div class="absolute inset-0 overflow-hidden blur-2xl scale-110 opacity-50" aria-hidden="true">
        <Artwork src={art} alt="" loading="eager" />
      </div>
    {:else}
      <div class="absolute inset-0" style="background: radial-gradient(circle at 30% 10%, var(--accent-wash-strong), transparent 60%), var(--void-800);"></div>
    {/if}
    <div
      class="detail-hero-scrim absolute inset-0"
      aria-hidden="true"
    ></div>
    <div class="absolute top-4 inset-x-0 z-10">
      <div class="max-w-[1440px] mx-auto px-6 md:px-8">
        <button class="btn-ghost px-3 py-1.5 text-xs" onclick={onBack}>
          <Icon name="arrowLeft" size={14} />
          Back
        </button>
      </div>
    </div>
  </div>

  <div class="px-6 md:px-8 pb-12 -mt-28 relative max-w-[1440px] mx-auto">
    <div class="flex flex-col sm:flex-row gap-6 items-start sm:items-end">
      <div
        class="w-[150px] sm:w-[190px] md:w-[224px] shrink-0 aspect-[2/3] rounded-2xl overflow-hidden relative"
        style="background: {media.coverImage?.color ?? 'var(--void-700)'}; border: 1px solid var(--line-strong); box-shadow: 0 24px 60px rgb(0 0 0 / 0.62), 0 0 0 1px rgb(255 255 255 / 0.04) inset;"
      >
        <Artwork src={art} alt="" loading="eager">
          {#snippet fallback()}
            <div class="w-full h-full grid place-items-center">
              <span class="text-5xl font-bold" style="color: var(--paper-500); font-family: var(--font-display);">
                {displayTitle(media).slice(0, 1)}
              </span>
            </div>
          {/snippet}
        </Artwork>
        <span class="absolute inset-0 pointer-events-none" style="box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08); border-radius: 16px;" aria-hidden="true"></span>
      </div>

      <div class="pb-1 min-w-0 flex-1">
        <p class="eyebrow">{media.format ?? "Title"} {media.status ? `· ${media.status.split("_").join(" ").toLowerCase()}` : ""}</p>
        <h1
          class="mt-1 leading-[1.02] line-clamp-2"
          style="font-family: var(--font-display); font-size: clamp(32px, 4vw, 54px); font-weight: 750; letter-spacing: -0.035em;"
        >
          {displayTitle(media)}
        </h1>
        {#if media.title.romaji && media.title.romaji !== displayTitle(media)}
          <p class="text-sm mt-1.5 line-clamp-1" style="color: var(--paper-400); font-family: var(--font-jp);">
            {media.title.romaji}
          </p>
        {/if}
        <p class="text-sm mt-2" style="color: var(--paper-300);">{metaOf(media)}</p>
        {#if nextAiring && nextLabel}
          <p
            class="text-sm mt-2 flex flex-wrap items-center gap-1.5"
            style="color: var(--paper-300);"
            title={nextLabel.aria}
          >
            <Icon name="calendar" size={13} />
            <span>Next episode EP {nextAiring.episode} · {formatScheduleDate(nextAiring.airingAt)} at {formatScheduleTime(nextAiring.airingAt)} · {nextLabel.text}</span>
          </p>
        {/if}

        {#if media.genres?.length}
          <div class="flex flex-wrap gap-1.5 mt-3">
            {#each media.genres as genre}
              <span class="chip text-[11px] px-2.5 py-1">{genre}</span>
            {/each}
          </div>
        {/if}

        <div class="flex flex-wrap items-center gap-2 mt-5">
          <button class="btn-primary px-5 py-2.5 text-sm" onclick={() => onPlay(media)}>
            <Icon name="play" size={14} />
            Watch
          </button>

          {#if inLibrary}
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <details class="status-menu" bind:this={menuEl} bind:open={menuOpen} onkeydown={onMenuKey}>
              <summary class="btn-ghost px-4 py-2.5 text-sm" aria-label="Library status, {statusLabel(status)}">
                <Icon name="check" size={14} />
                {statusLabel(status)}
                <Icon name="chevronRight" size={13} class="rotate-90" />
              </summary>
              <div class="menu" role="menu" aria-label="Library status">
                {#each statuses as entry}
                  <button
                    class="menu-item"
                    class:is-active={status === entry.id}
                    role="menuitem"
                    onclick={() => chooseStatus(entry.id)}
                  >
                    <span>{entry.label}</span>
                    {#if status === entry.id}
                      <Icon name="check" size={14} />
                    {/if}
                  </button>
                {/each}
                <div class="menu-rule"></div>
                <button class="menu-item is-danger" role="menuitem" onclick={() => onToggleLibrary(media)}>
                  <span>Remove from library</span>
                  <Icon name="xCircle" size={14} />
                </button>
              </div>
            </details>
          {:else}
            <button class="btn-ghost px-4 py-2.5 text-sm" onclick={() => onToggleLibrary(media)}>
              <Icon name="plus" size={14} />
              Library
            </button>
          {/if}
        </div>

        <div class="flex flex-wrap items-center gap-3 mt-4 text-[11px]" style="color: var(--paper-450);">
          <span class="inline-flex items-center gap-1.5">
            <Icon name="globe" size={12} />
            AniList metadata
          </span>
          <span class="inline-flex items-center gap-1.5">
            <Icon name="database" size={12} />
            Progress saved locally
          </span>
        </div>
      </div>

      {#if media.averageScore}
        <div class="hidden lg:flex flex-col items-center gap-2 shrink-0 pb-2">
          <div class="score-ring" style="--score: {media.averageScore}">
            <span class="score-ring-inner">
              <span class="font-display text-xl font-bold" style="font-family: var(--font-display);">{scoreOutOfTen(media.averageScore)}</span>
              <span class="text-[9px] tracking-wider" style="color: var(--paper-450);">/ 10</span>
            </span>
          </div>
          <span class="text-[9px] tracking-[0.16em] uppercase" style="color: var(--paper-450);">AniList score</span>
        </div>
      {/if}
    </div>

    <div class="mt-8 grid grid-cols-1 xl:grid-cols-[minmax(0,1fr)_340px] gap-8">
      <div class="min-w-0">
        <section aria-label="Synopsis">
          <h2 class="text-lg font-semibold mb-2">Synopsis</h2>
          <p class="text-sm leading-relaxed max-w-3xl" style="color: var(--paper-300);">
            {cleanHtml(media.description) || "AniList has no synopsis for this title yet."}
          </p>
        </section>

        <section class="mt-9" aria-label="Episodes">
          <div class="flex flex-wrap items-baseline gap-3 mb-3">
            <h2 class="text-lg font-semibold">Episodes</h2>
            {#if episodeCount > 0}
              <span class="text-sm" style="color: var(--paper-450);">
                {watchedCount} of {episodeCount} watched
              </span>
            {/if}
            <div class="ml-auto flex gap-1.5">
              <button
                class="btn-ghost px-2.5 py-1.5 text-[11px]"
                onclick={() => onMarkAll(true)}
                disabled={episodeCount === 0 || watchedCount === episodeCount}
              >
                Mark all
              </button>
              <button
                class="btn-ghost px-2.5 py-1.5 text-[11px]"
                onclick={() => onMarkAll(false)}
                disabled={watchedCount === 0}
              >
                Clear
              </button>
            </div>
          </div>

          {#if episodeCount > 0}
            <div class="surface p-4 mb-4">
              <div class="flex items-center gap-4">
                <div class="relative w-12 h-12 grid place-items-center rounded-full shrink-0" style="background: conic-gradient(var(--volt-400) {percent}%, var(--track) 0);">
                  <span class="absolute inset-[3px] rounded-full" style="background: var(--void-900);"></span>
                  <span class="relative font-mono text-[11px] font-semibold" style="color: var(--volt-200);">{percent}%</span>
                </div>
                <div class="min-w-0 flex-1">
                  <p class="text-sm font-semibold">{watchedCount === 0 ? "Ready to start" : watchedCount === episodeCount ? "Season complete" : "Keep the thread"}</p>
                  <p class="text-xs mt-0.5" style="color: var(--paper-450);">
                    {episodeCount - watchedCount} {episodeCount - watchedCount === 1 ? "episode" : "episodes"} left in this run
                  </p>
                </div>
              </div>
              <ProgressBar
                label="Episodes watched"
                value={watchedCount}
                max={episodeCount}
                size="md"
                class="mt-3"
              />
            </div>

            <ol class="grid grid-cols-5 sm:grid-cols-8 md:grid-cols-10 xl:grid-cols-12 gap-1.5" aria-label="Episodes">
              {#each Array(episodeCount) as _, index}
                {@const episode = index + 1}
                {@const seen = isWatched(episode)}
                <li>
                  <button
                    class="episode-tile w-full {seen ? 'is-seen' : ''}"
                    onclick={() => onToggleWatched(episode)}
                    aria-pressed={seen}
                    aria-label={seen ? `Mark episode ${episode} unwatched` : `Mark episode ${episode} watched`}
                    title={seen ? `Mark episode ${episode} unwatched` : `Mark episode ${episode} watched`}
                  >
                    {episode}
                    {#if seen}
                      <span class="absolute top-1 right-1" aria-hidden="true">
                        <Icon name="check" size={9} strokeWidth={2.2} />
                      </span>
                    {/if}
                  </button>
                </li>
              {/each}
            </ol>
            <p class="text-xs mt-3" style="color: var(--paper-450);">
              Click an episode to mark it watched. Watch finds this title in an installed source.
            </p>
            {#if (media.episodes ?? 0) > 200}
              <p class="text-xs mt-1" style="color: var(--paper-450);">
                Showing the first 200 of {media.episodes} listed episodes.
              </p>
            {/if}
          {:else}
            <div class="surface px-4 py-4 text-sm" style="color: var(--paper-450);">
              AniList has no episode count for this title.
            </div>
          {/if}
        </section>
      </div>

      <aside class="self-start space-y-4" aria-label="Details">
        {#if relatedSeasons.length > 0}
          <div class="surface p-4">
            <h2 class="text-[10px] font-semibold uppercase tracking-[0.18em] mb-3" style="color: var(--paper-450);">
              Seasons
            </h2>
            <ul class="space-y-2">
              {#each relatedSeasons as season (season.item.id)}
                {@const saved = libraryIds.has(season.item.id)}
                {@const seasonArt = coverOf(season.item)}
                <li class="flex items-center gap-2">
                  <button
                    class="flex min-w-0 flex-1 items-center gap-3 rounded-xl p-1 text-left"
                    onclick={() => onOpenRecommendation(season.item)}
                    aria-label="Open {displayTitle(season.item)}"
                  >
                    <span class="w-[40px] h-[56px] shrink-0 overflow-hidden rounded-lg" style="background: var(--void-700);">
                      <Artwork src={seasonArt} alt="" loading="lazy">
                        {#snippet fallback()}
                          <span class="w-full h-full grid place-items-center text-lg font-bold" style="color: var(--paper-500); font-family: var(--font-display);">
                            {displayTitle(season.item).slice(0, 1)}
                          </span>
                        {/snippet}
                      </Artwork>
                    </span>
                    <span class="min-w-0 flex-1">
                      <span class="block text-sm font-semibold leading-snug truncate">{displayTitle(season.item)}</span>
                      <span class="block text-[11px] mt-0.5" style="color: var(--volt-300);">
                        {seasonLabels[season.relation] ?? season.relation}
                      </span>
                      <span class="block text-[11px] mt-0.5 truncate" style="color: var(--paper-450);">
                        {metaOf(season.item) || "n/a"}
                      </span>
                    </span>
                  </button>
                  <button
                    class="btn-ghost px-2.5 py-1.5 text-[11px] shrink-0"
                    onclick={() => onToggleLibrary(season.item)}
                    aria-pressed={saved}
                    aria-label={saved ? `Remove ${displayTitle(season.item)} from library` : `Save ${displayTitle(season.item)} to library`}
                    title={saved ? "Remove from library" : "Save to library"}
                  >
                    <Icon name={saved ? "check" : "plus"} size={12} />
                    {saved ? "Saved" : "Add"}
                  </button>
                </li>
              {/each}
            </ul>
          </div>
        {/if}
        <div class="surface p-4">
          <h2 class="text-[10px] font-semibold uppercase tracking-[0.18em] mb-3" style="color: var(--paper-450);">
            Details
          </h2>
          <dl class="space-y-2.5">
            <div class="flex justify-between gap-3 text-sm">
              <dt style="color: var(--paper-450);">Score</dt>
              <dd class="font-semibold" style="font-family: var(--font-mono);" title={media.averageScore ? `AniList score ${media.averageScore} out of 100` : undefined}>{scoreLabel(media.averageScore)}</dd>
            </div>
            <div class="flex justify-between gap-3 text-sm">
              <dt style="color: var(--paper-450);">Format</dt>
              <dd>{media.format ?? "n/a"}</dd>
            </div>
            <div class="flex justify-between gap-3 text-sm">
              <dt style="color: var(--paper-450);">Status</dt>
              <dd>{media.status ? media.status.split("_").join(" ").toLowerCase() : "n/a"}</dd>
            </div>
            <div class="flex justify-between gap-3 text-sm">
              <dt style="color: var(--paper-450);">Episodes</dt>
              <dd style="font-family: var(--font-mono);">{media.episodes ?? "n/a"}</dd>
            </div>
            <div class="flex justify-between gap-3 text-sm">
              <dt style="color: var(--paper-450);">Year</dt>
              <dd style="font-family: var(--font-mono);">{media.seasonYear ?? "n/a"}</dd>
            </div>
            <div class="flex justify-between gap-3 text-sm">
              <dt style="color: var(--paper-450);">AniList id</dt>
              <dd style="font-family: var(--font-mono);">{media.id}</dd>
            </div>
          </dl>
        </div>

        <div class="surface p-4">
          <div class="flex items-center justify-between gap-3">
            <h2 class="text-[10px] font-semibold uppercase tracking-[0.18em]" style="color: var(--paper-450);">
              Progress
            </h2>
            <span class="chip chip-volt text-[10px] px-2 py-0.5">{percent}%</span>
          </div>
          <p class="mt-3 font-display text-2xl font-bold" style="font-family: var(--font-display);">
            {watchedCount} <span class="text-sm font-medium" style="color: var(--paper-450);">/ {episodeCount || "?"}</span>
          </p>
          <p class="text-xs mt-1" style="color: var(--paper-450);">episodes watched</p>
          <div class="h-1 rounded-full mt-3 overflow-hidden" style="background: var(--void-800);" aria-hidden="true">
            <div class="h-full" style="width: {percent}%; background: var(--volt-400);"></div>
          </div>
        </div>
      </aside>
    </div>

    {#if recommendations.length > 0}
      <section class="mt-10" aria-label="More like this">
        <div class="flex items-baseline gap-3 mb-3">
          <h2 class="shelf-title">More like this</h2>
          <span class="shelf-count">{recommendations.length}</span>
        </div>
        <div class="stagger-grid grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-6 gap-4">
          {#each recommendations as item (item.id)}
            <article class="poster-card group">
              <div class="poster-art aspect-[2/3]">
                <button
                  class="absolute inset-0 w-full h-full text-left"
                  onclick={() => onOpenRecommendation(item)}
                  aria-label="Open {displayTitle(item)}"
                >
                  <Artwork src={coverOf(item)} alt="" loading="lazy">
                    {#snippet fallback()}
                      <span
                        class="w-full h-full grid place-items-center"
                        style="background: radial-gradient(circle at 30% 20%, var(--accent-wash-strong), transparent 60%), var(--void-800);"
                      >
                        <span class="text-4xl font-bold" style="color: var(--paper-500); font-family: var(--font-display);">
                          {displayTitle(item).slice(0, 1)}
                        </span>
                      </span>
                    {/snippet}
                  </Artwork>
                  <span class="poster-scrim" aria-hidden="true"></span>
                </button>
              </div>
              <button class="poster-body" onclick={() => onOpenRecommendation(item)}>
                <span class="poster-title">{displayTitle(item)}</span>
                <span class="poster-meta">{metaOf(item)}</span>
              </button>
            </article>
          {/each}
        </div>
      </section>
    {/if}
  </div>
</div>

<style>
  .status-menu {
    position: relative;
  }

  .status-menu summary {
    list-style: none;
  }

  .status-menu summary::-webkit-details-marker {
    display: none;
  }

  .status-menu[open] summary {
    border-color: var(--line-volt);
    background: var(--accent-wash);
  }

  .status-menu .menu {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    z-index: 30;
    min-width: 200px;
    padding: 6px;
    border-radius: 14px;
    background: var(--glass-strong);
    border: 1px solid var(--line-strong);
    box-shadow: var(--shadow-pop);
    backdrop-filter: blur(24px) saturate(150%);
    -webkit-backdrop-filter: blur(24px) saturate(150%);
    animation: menuIn 120ms var(--ease-out) both;
  }

  @keyframes menuIn {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .menu-item {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 9px;
    color: var(--paper-300);
    font-size: 12px;
    font-weight: 550;
    text-align: left;
    transition: background 120ms ease-out, color 120ms ease-out;
  }

  .menu-item:hover {
    background: var(--tint-1);
    color: var(--text-1);
  }

  .menu-item.is-active {
    background: var(--accent-wash);
    color: var(--volt-200);
  }

  .menu-item.is-danger {
    color: var(--danger);
  }

  .menu-item.is-danger:hover {
    background: var(--danger-wash);
    color: var(--danger);
  }

  .menu-rule {
    height: 1px;
    margin: 5px 6px;
    background: var(--line);
  }

  @media (prefers-reduced-motion: reduce) {
    .status-menu .menu {
      animation: none;
    }
  }
</style>
