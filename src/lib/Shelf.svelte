<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    count,
    onSeeAll,
    resetKey = 0,
    children,
  }: {
    title: string;
    count?: number;
    onSeeAll?: () => void;
    resetKey?: string | number;
    children: Snippet;
  } = $props();

  // Snippet children render below through {@render children()}.
  let row: HTMLDivElement | null = $state(null);

  // Data arrives after the skeleton. Without this the browser keeps the old
  // horizontal anchor and can land at the end of a freshly filled row.
  $effect(() => {
    resetKey;
    if (row) row.scrollLeft = 0;
  });

  const reduceMotion =
    typeof matchMedia !== "undefined" &&
    matchMedia("(prefers-reduced-motion: reduce)").matches;

  function page(dir: 1 | -1) {
    if (!row) return;
    row.scrollBy({
      left: dir * row.clientWidth * 0.8,
      behavior: reduceMotion ? "auto" : "smooth",
    });
  }

  function onKeys(event: KeyboardEvent) {
    if (event.key === "ArrowRight") {
      event.preventDefault();
      page(1);
    } else if (event.key === "ArrowLeft") {
      event.preventDefault();
      page(-1);
    }
  }
</script>

<section class="shelf-hover" aria-label={title}>
  <div class="shelf-head">
    <h2 class="shelf-title">{title}</h2>
    {#if count !== undefined}
      <span class="shelf-count">{count}</span>
    {/if}
    <div class="flex-1"></div>
    {#if onSeeAll}
      <button class="flex items-center gap-1 text-xs font-semibold" style="color: var(--volt-300);" onclick={onSeeAll}>
        See all
        <Icon name="arrowRight" size={13} />
      </button>
    {/if}
    <div class="shelf-nav">
      <button class="shelf-arrow" aria-label="Scroll {title} back" onclick={() => page(-1)}>
        <Icon name="chevronLeft" size={15} />
      </button>
      <button class="shelf-arrow" aria-label="Scroll {title} forward" onclick={() => page(1)}>
        <Icon name="chevronRight" size={15} />
      </button>
    </div>
  </div>
  <!-- The scroll row is focusable so the arrow keys can page it. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={row}
    class="shelf-scroll"
    tabindex={0}
    role="group"
    aria-label="{title} row. Use left and right arrows to scroll."
    onkeydown={onKeys}
  >
    {@render children()}
  </div>
</section>
