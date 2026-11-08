<script lang="ts">
  let { kind }: { kind: "hero" | "grid" | "detail" | "row" | "list" } = $props();
</script>

{#if kind === "hero"}
  <div class="relative h-[clamp(340px,38vw,430px)] overflow-hidden" style="border-bottom: 1px solid var(--line);" aria-label="Loading highlights">
    <div class="shimmer absolute inset-0"></div>
    <div class="absolute inset-0" style="background: linear-gradient(90deg, var(--void-950) 0%, rgb(8 8 15 / 0.88) 34%, transparent 78%);"></div>
    <div class="relative h-full max-w-[1440px] mx-auto px-6 md:px-8 flex flex-col justify-center pb-10 space-y-4">
      <div class="shimmer h-3 w-40 rounded"></div>
      <div class="shimmer h-11 w-[min(520px,70%)] rounded-xl"></div>
      <div class="shimmer h-3 w-[min(360px,54%)] rounded"></div>
      <div class="flex gap-2 pt-2">
        <div class="shimmer h-9 w-28 rounded-xl"></div>
        <div class="shimmer h-9 w-28 rounded-xl"></div>
      </div>
    </div>
  </div>
{:else if kind === "detail"}
  <div aria-label="Loading details">
    <div class="shimmer h-[320px]"></div>
    <div class="px-6 md:px-8 -mt-28 relative max-w-[1440px] mx-auto flex gap-6 items-end">
      <div class="shimmer w-[168px] md:w-[224px] aspect-[2/3] rounded-2xl shrink-0"></div>
      <div class="flex-1 space-y-3 pb-2">
        <div class="shimmer h-3 w-32 rounded"></div>
        <div class="shimmer h-10 w-2/3 rounded-xl"></div>
        <div class="shimmer h-3 w-1/3 rounded"></div>
        <div class="flex gap-2 pt-2">
          <div class="shimmer h-9 w-24 rounded-xl"></div>
          <div class="shimmer h-9 w-24 rounded-xl"></div>
        </div>
      </div>
    </div>
    <div class="px-6 md:px-8 py-10 max-w-[1440px] mx-auto grid grid-cols-1 xl:grid-cols-3 gap-8">
      <div class="xl:col-span-2 space-y-3">
        <div class="shimmer h-4 w-28 rounded"></div>
        <div class="shimmer h-3 w-full rounded"></div>
        <div class="shimmer h-3 w-11/12 rounded"></div>
        <div class="shimmer h-3 w-4/5 rounded"></div>
      </div>
      <div class="shimmer h-48 rounded-2xl"></div>
    </div>
  </div>
{:else if kind === "row"}
  <div class="flex gap-4 overflow-hidden" aria-label="Loading row">
    {#each Array(7) as _}
      <div class="w-[164px] lg:w-[184px] shrink-0 space-y-2">
        <div class="shimmer aspect-[2/3] rounded-2xl"></div>
        <div class="shimmer h-3 w-3/4 rounded"></div>
        <div class="shimmer h-2 w-1/2 rounded"></div>
      </div>
    {/each}
  </div>
{:else if kind === "list"}
  <div class="space-y-2" aria-label="Loading list">
    {#each Array(5) as _}
      <div class="surface p-3 flex items-center gap-3">
        <div class="shimmer w-24 h-14 rounded-lg shrink-0"></div>
        <div class="flex-1 space-y-2">
          <div class="shimmer h-3 w-1/2 rounded"></div>
          <div class="shimmer h-2 w-1/3 rounded"></div>
        </div>
        <div class="shimmer h-8 w-20 rounded-lg"></div>
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
