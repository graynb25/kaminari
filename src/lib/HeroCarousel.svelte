<script lang="ts">
  import { coverOf, cleanHtml, displayTitle, metaOf, scoreOutOfTen } from "./format";
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

  const slides = $derived(items.slice(0, 5));
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
</script>

{#if slides.length > 0}
  <section
    class="hero-edge relative h-[clamp(320px,38vw,440px)] overflow-hidden shrink-0"
    style="border-bottom: 1px solid var(--line);"
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
      {@const art = slide.bannerImage || coverOf(slide)}
      <div class="hero-slide {active ? 'active' : ''}" aria-hidden={active ? undefined : "true"}>
        {#if art}
          <img
            src={art}
            alt=""
            class="kenburns absolute inset-0 w-full h-full object-cover"
            loading={position === 0 ? "eager" : "lazy"}
          />
        {:else}
          <div class="absolute inset-0" style="background: var(--void-800);"></div>
        {/if}
        <div
          class="absolute inset-0"
          style="background: linear-gradient(90deg, var(--void-950) 20%, rgb(8 8 15 / 0.85) 50%, rgb(8 8 15 / 0.25) 75%, transparent 100%), linear-gradient(0deg, var(--void-950) 10%, transparent 50%);"
          aria-hidden="true"
        ></div>
      </div>
    {/each}

    <div class="absolute inset-0 pointer-events-none">
      <div class="relative h-full max-w-[1440px] mx-auto px-6 md:px-8">
        {#each slides as slide, position (slide.id)}
          {@const active = position === index % slides.length}
          <div
            class="hero-copy {active ? 'active' : ''} absolute inset-y-0 left-6 md:left-8 right-6 flex flex-col justify-center max-w-2xl min-w-0"
            aria-hidden={active ? undefined : "true"}
            inert={!active}
          >
              <p class="eyebrow">#{position + 1} Trending{slide.averageScore ? ` · ${scoreOutOfTen(slide.averageScore)}/10` : ""}</p>
              <h1
                class="mt-3 leading-[1.08] line-clamp-2"
                style="font-family: var(--font-display); font-size: clamp(28px, 3.4vw, 40px); font-weight: 700; letter-spacing: -0.02em;"
              >
                {displayTitle(slide)}
              </h1>
              <p class="text-sm mt-2" style="color: var(--paper-300);">{metaOf(slide)}</p>
              <p class="text-sm mt-3 line-clamp-2 max-w-xl leading-relaxed" style="color: var(--paper-300);">
                {cleanHtml(slide.description) || "AniList has no synopsis for this title yet."}
              </p>
              <div class="flex flex-wrap gap-2 mt-5">
                <button class="btn-primary px-4 py-2 text-sm" onclick={() => onOpen(slide)}>
                  ▶ Play
                </button>
                <button class="btn-ghost px-4 py-2 text-sm" onclick={() => onToggleLibrary(slide)}>
                  {isInLibrary(slide.id) ? "✓ In Library" : "+ Library"}
                </button>
                <button class="btn-ghost px-4 py-2 text-sm" onclick={() => onOpen(slide)}>
                  Details
                </button>
              </div>
            </div>
        {/each}

        <div class="absolute bottom-4 left-6 md:left-8 flex items-center gap-2 pointer-events-auto">
          {#each slides as slide, position (slide.id)}
            <button
              class="h-1.5 rounded-full"
              style="width: {position === index % slides.length ? '28px' : '8px'}; background: {position === index % slides.length ? 'var(--volt-400)' : 'var(--void-700)'}; transition: width 250ms var(--ease-out), background 250ms var(--ease-out);"
              aria-label="Go to slide {position + 1}: {displayTitle(slide)}"
              aria-current={position === index % slides.length ? "true" : undefined}
              onclick={() => go(position)}
            ></button>
          {/each}
        </div>

        <div class="absolute bottom-3 right-6 md:right-8 flex gap-1.5 pointer-events-auto">
          <button
            class="w-8 h-8 grid place-items-center rounded-lg"
            style="background: rgb(8 8 15 / 0.72); border: 1px solid var(--line); backdrop-filter: blur(12px); color: var(--paper-50);"
            aria-label="Previous highlight"
            onclick={() => go(index - 1)}
          >
            ‹
          </button>
          <button
            class="w-8 h-8 grid place-items-center rounded-lg"
            style="background: rgb(8 8 15 / 0.72); border: 1px solid var(--line); backdrop-filter: blur(12px); color: var(--paper-50);"
            aria-label="Next highlight"
            onclick={() => go(index + 1)}
          >
            ›
          </button>
        </div>
      </div>
    </div>
  </section>
{/if}
