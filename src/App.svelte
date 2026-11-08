<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  type Shelves = [string, string[]][];

  let title = $state("Kaminari (雷)");
  let shelfData = $state<Shelves>([
    ["Trending", []],
  ]);
  let error = $state<string | null>(null);
  let ready = $state(false);

  async function boot() {
    try {
      const res = await invoke<{ title: string; shelves: Shelves }>("boot_snapshot");
      title = res.title;
      shelfData = res.shelves;
      ready = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      // Offline or fresh install path. Show empty shelves, not a blank screen.
      shelfData = [["Trending", []]];
      ready = true;
    }
  }

  onMount(() => {
    void boot();
    const off = listen("kaminari/v1/library_changed", () => {
      void boot();
    });
    return () => { void off.then(u => u()); };
  });
</script>

<div class="h-screen flex flex-col">
  <header class="h-10 flex items-center justify-between px-4" style="background: var(--void-900); border-bottom: 1px solid var(--line);">
    <div class="flex items-center gap-3">
      <span class="font-semibold tracking-wide">⚡ {title}</span>
      <nav class="flex gap-2 text-sm" style="color: var(--paper-300);">
        <a href="/library" class="px-2 py-1 rounded-md" style="color: var(--paper-50);">Library</a>
        <a href="/browse" class="px-2 py-1">Browse</a>
        <a href="/sources" class="px-2 py-1">Sources</a>
      </nav>
    </div>
    <button
      class="w-[280px] h-7 rounded-md text-left px-3 text-sm"
      style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-500);"
      onclick={() => document.dispatchEvent(new KeyboardEvent("keydown", { key: "k" }))}
    >
      Search <span class="float-right">Ctrl K</span>
    </button>
  </header>

  <main class="flex-1 overflow-y-auto p-6">
    {#if !ready}
      <div class="space-y-4">
        <div class="h-48 rounded-xl animate-pulse" style="background: var(--void-800);"></div>
        <div class="grid grid-cols-6 gap-4">
          <div class="aspect-[2/3] rounded-md animate-pulse" style="background: var(--void-800);"></div>
          <div class="aspect-[2/3] rounded-md animate-pulse" style="background: var(--void-800);"></div>
          <div class="aspect-[2/3] rounded-md animate-pulse" style="background: var(--void-800);"></div>
        </div>
      </div>
    {:else if error}
      <div class="p-4 rounded-lg" style="background: var(--void-800); border: 1px solid var(--line);">
        <p>Could not load from the local core yet.</p>
        <p class="text-sm mt-1" style="color: var(--paper-300);">{error}</p>
        <button class="mt-3 px-3 py-1.5 rounded-md" style="background: var(--volt-500);" onclick={() => boot()}>Retry</button>
      </div>
    {:else}
      {#each shelfData as [name, items]}
        <section class="mb-8">
          <div class="flex items-baseline gap-3 mb-3">
            <h2 class="text-lg font-semibold">{name}</h2>
            <span class="text-sm" style="color: var(--paper-500);">{items.length} titles</span>
            <a href="/browse" class="ml-auto text-sm" style="color: var(--volt-400);">See all</a>
          </div>
          {#if items.length === 0}
            <p class="text-sm" style="color: var(--paper-300);">No results yet. Press Ctrl K to search AniList.</p>
          {:else}
            <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
              {#each items as titleStr}
                <article class="poster-card rounded-md overflow-hidden cursor-pointer" style="background: var(--void-800); border: 1px solid var(--line);">
                  <div class="aspect-[2/3]" style="background: var(--void-700);"></div>
                  <div class="p-2">
                    <p class="text-sm font-semibold truncate">{titleStr}</p>
                    <p class="text-xs" style="color: var(--paper-500);">2026</p>
                  </div>
                </article>
              {/each}
            </div>
          {/if}
        </section>
      {/each}
    {/if}
  </main>
</div>
