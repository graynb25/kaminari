<script lang="ts">
  import type { Snippet } from "svelte";

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

  function page(dir: 1 | -1) {
    if (!row) return;
    row.scrollBy({ left: dir * row.clientWidth * 0.8, behavior: "smooth" });
  }
</script>

<section class="shelf-hover" aria-label={title}>
  <div class="flex items-baseline gap-3 mb-3">
    <h2 class="text-xl font-semibold" style="font-family: var(--font-display);">{title}</h2>
    {#if count !== undefined}
      <span class="text-sm" style="color: var(--paper-500);">{count}</span>
    {/if}
    <div class="flex-1"></div>
    {#if onSeeAll}
      <button
        class="text-sm font-semibold"
        style="color: var(--volt-300);"
        onclick={onSeeAll}
      >
        See all
      </button>
    {/if}
    <div class="shelf-nav flex gap-1">
      <button
        class="w-7 h-7 grid place-items-center rounded-lg text-sm"
        style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
        aria-label="Scroll {title} back"
        onclick={() => page(-1)}
      >
        ‹
      </button>
      <button
        class="w-7 h-7 grid place-items-center rounded-lg text-sm"
        style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-300);"
        aria-label="Scroll {title} forward"
        onclick={() => page(1)}
      >
        ›
      </button>
    </div>
  </div>
  <div
    bind:this={row}
    class="shelf-scroll flex gap-4 overflow-x-auto pb-2"
  >
    {@render children()}
  </div>
</section>
