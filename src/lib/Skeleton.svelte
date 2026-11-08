<script lang="ts">
  let { kind }: { kind: "hero" | "grid" | "detail" | "row" } = $props();
</script>

{#if kind === "hero"}
  <div class="shimmer h-[400px] shrink-0" style="border-bottom: 1px solid var(--line);" aria-label="Loading highlights"></div>
{:else if kind === "detail"}
  <div class="animate-pulse" aria-label="Loading details">
    <div class="shimmer h-[320px]"></div>
    <div class="px-8 -mt-24 relative flex gap-6 items-end">
      <div class="shimmer w-[224px] aspect-[2/3] rounded-2xl shrink-0"></div>
      <div class="flex-1 space-y-3 pb-2">
        <div class="shimmer h-9 w-2/3 rounded-lg"></div>
        <div class="shimmer h-4 w-1/3 rounded"></div>
        <div class="flex gap-2">
          <div class="shimmer h-9 w-24 rounded-lg"></div>
          <div class="shimmer h-9 w-24 rounded-lg"></div>
        </div>
      </div>
    </div>
  </div>
{:else if kind === "row"}
  <div class="flex gap-4 overflow-hidden" aria-label="Loading row">
    {#each Array(6) as _}
      <div class="w-[160px] shrink-0 space-y-2">
        <div class="shimmer aspect-[2/3] rounded-2xl"></div>
        <div class="shimmer h-3 w-3/4 rounded"></div>
      </div>
    {/each}
  </div>
{:else}
  <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4" aria-label="Loading titles">
    {#each Array(12) as _}
      <div class="space-y-2">
        <div class="shimmer aspect-[2/3] rounded-2xl"></div>
        <div class="shimmer h-3 w-3/4 rounded"></div>
        <div class="shimmer h-2 w-1/2 rounded"></div>
      </div>
    {/each}
  </div>
{/if}
