<script lang="ts">
  import Icon from "./Icon.svelte";
  import { coverOf, cleanHtml, displayTitle, scoreOutOfTen } from "./format";
  import type { Media } from "./types";

  let {
    items,
    isInLibrary,
    onOpen,
    onToggleLibrary,
    onPlay,
  }: {
    items: Media[];
    isInLibrary: (id: number) => boolean;
    onOpen: (item: Media) => void;
    onToggleLibrary: (item: Media) => void;
    onPlay: (item: Media) => void;
  } = $props();

  // The hero only uses titles with a real banner. A clean full-bleed image with
  // the slow zoom reads better than a blurred cover, and the shelves still show
  // titles without one.
  const slides = $derived.by(() => {
    const withBanners = items.filter((item) => !!item.bannerImage);
    return withBanners.length >= 3 ? withBanners.slice(0, 5) : items.slice(0, 5);
  });
  let index = $state(0);
  let paused = $state(false);
  let resumeTimer: number | null = $state(null);

  const reduceMotion =
    typeof matchMedia !== "undefined" &&
    matchMedia("(prefers-reduced-motion: reduce)").matches;

  // Interval restarts on index change, so each slide holds a full 7 seconds.
  $effect(() => {
    if (slides.length < 2 || paused || reduceMotion) return;
    const current = index;
    const timer = window.setInterval(() => {
      index = (current + 1) % slides.length;
    }, 7000);
    return () => window.clearInterval(timer);
  });

  function pause() {
    paused = true;
    if (resumeTimer !== null) window.clearTimeout(resumeTimer);
  }

  function resume() {
    if (resumeTimer !== null) window.clearTimeout(resumeTimer);
    resumeTimer = window.setTimeout(() => {
      paused = false;
    }, 3000);
  }

  function go(next: number) {
    pause();
    index = (next + slides.length) % slides.length;
    resume();
  }

  function onKeys(event: KeyboardEvent) {
    if (event.key === "ArrowRight") {
      event.preventDefault();
      go(index + 1);
    } else if (event.key === "ArrowLeft") {
      event.preventDefault();
      go(index - 1);
    }
  }

  function rankOf(slide: Media): number {
    const rank = items.findIndex((item) => item.id === slide.id);
    return rank >= 0 ? rank + 1 : 1;
  }

  function seasonLabel(): string {
    const now = new Date();
    const month = now.getMonth();
    const season = month < 2 || month === 11 ? "Winter" : month < 5 ? "Spring" : month < 8 ? "Summer" : "Fall";
    return `${season} ${now.getFullYear()}`;
  }

  function statusLabel(status?: string | null): string {
    if (status === "RELEASING") return "Airing";
    if (status === "FINISHED") return "Finished";
    if (status === "NOT_YET_RELEASED") return "Upcoming";
    return status ? status.toLowerCase() : "";
  }
</script>

{#if slides.length > 0}
  <!-- The carousel is a labelled region with explicit arrow key handling. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <section
    class="hero-shell hero-edge"
    aria-label="Trending highlights"
    aria-roledescription="carousel"
    tabindex={0}
    onkeydown={onKeys}
    onmouseenter={pause}
    onmouseleave={resume}
    onfocusin={pause}
    onfocusout={resume}
    onpointerdown={pause}
  >
    {#each slides as slide, position (slide.id)}
      {@const active = position === index % slides.length}
      {@const banner = slide.bannerImage}
      {@const cover = coverOf(slide)}
      <div class="hero-slide {active ? 'active' : ''}" aria-hidden={active ? undefined : "true"}>
        {#if banner}
          <img
            src={banner}
            alt=""
            class="kenburns absolute inset-0 w-full h-full object-cover"
            loading={position === 0 ? "eager" : "lazy"}
            decoding="async"
          />
        {:else if cover}
          <!-- Missing banner: blurred cover for color, plus a clear poster card below. -->
          <img
            src={cover}
            alt=""
            class="kenburns absolute inset-0 w-full h-full object-cover blur-xl scale-110 opacity-45"
            loading={position === 0 ? "eager" : "lazy"}
            decoding="async"
          />
        {:else}
          <div class="absolute inset-0" style="background: var(--void-800);"></div>
        {/if}
        <div class="hero-scrim" aria-hidden="true"></div>
      </div>
    {/each}

    <div class="absolute inset-0 pointer-events-none">
      <div class="relative h-full max-w-[1440px] mx-auto">
        {#each slides as slide, position (slide.id)}
          {@const active = position === index % slides.length}
          {@const banner = slide.bannerImage}
          {@const cover = coverOf(slide)}
          {#if !banner && cover}
            <div class="hero-poster {active ? 'active' : ''}" aria-hidden="true">
              <img
                src={cover}
                alt=""
                loading={position === 0 ? "eager" : "lazy"}
                decoding="async"
              />
            </div>
          {/if}
        {/each}

        {#each slides as slide, position (slide.id)}
          {@const active = position === index % slides.length}
          <div
            class="hero-copy {active ? 'active' : ''}"
            aria-hidden={active ? undefined : "true"}
            inert={!active}
          >
            <p class="hero-eyebrow">
              <span class="live-dot" aria-hidden="true"></span>
              #{rankOf(slide)} Trending
              <span style="color: var(--paper-500);">/</span>
              {seasonLabel()}
            </p>
            <h1 class="hero-title line-clamp-2">{displayTitle(slide)}</h1>
            {#if slide.title.romaji && slide.title.romaji !== displayTitle(slide)}
              <p class="hero-subtitle line-clamp-1">{slide.title.romaji}</p>
            {/if}
            <div class="hero-meta">
              {#if slide.averageScore}
                <span class="hero-meta-item is-score" title="AniList score {slide.averageScore} out of 100">
                  <Icon name="star" size={11} />
                  {scoreOutOfTen(slide.averageScore)}/10
                </span>
              {/if}
              {#if slide.seasonYear}
                <span class="hero-meta-item">{slide.seasonYear}</span>
              {/if}
              {#if slide.format}
                <span class="hero-meta-item">{slide.format}</span>
              {/if}
              {#if slide.episodes}
                <span class="hero-meta-item">{slide.episodes} eps</span>
              {/if}
              {#if slide.status}
                <span class="hero-meta-item">{statusLabel(slide.status)}</span>
              {/if}
            </div>
            <p class="hero-synopsis">
              {cleanHtml(slide.description) || "AniList has no synopsis for this title yet."}
            </p>
            <div class="hero-actions">
              <button class="btn-primary px-4 py-2.5 text-sm" onclick={() => onPlay(slide)}>
                <Icon name="play" size={14} />
                Play
              </button>
              <button class="btn-ghost px-4 py-2.5 text-sm" onclick={() => onToggleLibrary(slide)}>
                <Icon name={isInLibrary(slide.id) ? "check" : "plus"} size={14} />
                {isInLibrary(slide.id) ? "In Library" : "Library"}
              </button>
              <button class="btn-ghost px-4 py-2.5 text-sm" onclick={() => onOpen(slide)}>
                Details
                <Icon name="arrowRight" size={14} />
              </button>
            </div>
          </div>
        {/each}

        <div class="hero-controls">
          <div class="hero-dots">
            {#each slides as slide, position (slide.id)}
              <button
                class="hero-dot {position === index % slides.length ? 'active' : ''}"
                aria-label="Go to slide {position + 1}: {displayTitle(slide)}"
                aria-current={position === index % slides.length ? "true" : undefined}
                onclick={() => go(position)}
              ></button>
            {/each}
          </div>
          <span class="hero-counter">
            {String((index % slides.length) + 1).padStart(2, "0")} / {String(slides.length).padStart(2, "0")}
          </span>
          <div class="hero-arrows">
            <button class="hero-arrow" aria-label="Previous highlight" onclick={() => go(index - 1)}>
              <Icon name="chevronLeft" size={16} />
            </button>
            <button class="hero-arrow" aria-label="Next highlight" onclick={() => go(index + 1)}>
              <Icon name="chevronRight" size={16} />
            </button>
          </div>
        </div>
      </div>
    </div>
  </section>
{/if}
