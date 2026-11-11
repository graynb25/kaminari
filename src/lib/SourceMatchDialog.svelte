<script lang="ts">
  import { onMount } from "svelte";
  import Artwork from "./Artwork.svelte";
  import Callout from "./Callout.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import { jsExtensionList, jsExtensionTest } from "./api";
  import { displayTitle } from "./format";
  import { filterNsfwSources, showNsfwEnabled } from "./nsfw";
  import { sortByMatch } from "./sourceMatch";
  import type { JsExtension, JsSearchItem, Media } from "./types";

  type Group = {
    extension: JsExtension;
    items: JsSearchItem[];
    loading: boolean;
    error: string | null;
  };

  let {
    media,
    onClose,
    onOpen,
    onOpenSources,
  }: {
    media: Media;
    onClose: () => void;
    onOpen: (extension: JsExtension, item: JsSearchItem) => void;
    onOpenSources: () => void;
  } = $props();

  let extensions = $state<JsExtension[]>([]);
  let groups = $state<Group[]>([]);
  let query = $state("");
  let loading = $state(true);
  let error = $state<string | null>(null);
  let hiddenNsfw = $state(0);
  let disposed = false;

  // Seed the search box from the selected title. A later media change (for
  // example a refreshed AniList row) updates it; typing only changes query.
  $effect.pre(() => {
    query = displayTitle(media);
  });

  onMount(() => {
    void load();
    return () => {
      disposed = true;
    };
  });

  function errorText(value: unknown): string {
    return value instanceof Error ? value.message : String(value);
  }

  async function load() {
    // The pre-effect seeds the box before paint; this also covers the mount
    // ordering if load starts first.
    if (!query.trim()) query = displayTitle(media);
    loading = true;
    error = null;
    try {
      const installed = (await jsExtensionList()).filter((extension) => extension.enabled);
      extensions = filterNsfwSources(installed, showNsfwEnabled());
      hiddenNsfw = installed.length - extensions.length;
    } catch (e) {
      error = errorText(e);
      loading = false;
      return;
    }
    if (extensions.length === 0) {
      loading = false;
      return;
    }
    await search();
  }

  async function search() {
    const text = query.trim();
    if (!text) {
      error = "Type a title to search.";
      return;
    }
    error = null;
    loading = true;
    groups = extensions.map((extension) => ({
      extension,
      items: [],
      loading: true,
      error: null,
    }));
    await Promise.all(
      groups.map(async (group) => {
        try {
          const result = await jsExtensionTest(group.extension.id, text);
          group.items = sortByMatch(result.items ?? [], text);
        } catch (e) {
          group.error = errorText(e);
        } finally {
          group.loading = false;
          if (!disposed) groups = [...groups];
        }
      }),
    );
    if (!disposed) loading = false;
  }
</script>

<Modal
  size="lg"
  maxHeight="88vh"
  overlayName="source-match"
  label="Find {displayTitle(media)} in a source"
  onClose={onClose}
  headerClass="flex items-center gap-3 px-5 py-4"
  bodyClass="px-5 py-4"
>
  {#snippet header()}
    <span
      class="w-9 h-9 grid place-items-center rounded-xl shrink-0"
      style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);"
    >
      <Icon name="play" size={17} />
    </span>
    <div class="min-w-0 flex-1">
      <h2 class="text-base font-semibold">Find a source</h2>
      <p class="text-[11px] truncate" style="color: var(--text-4);">{displayTitle(media)}</p>
    </div>
    <button class="btn-ghost px-3 py-1.5 text-xs" onclick={onClose}>
      Close
      <kbd class="kbd">Esc</kbd>
    </button>
  {/snippet}

  <form
    class="flex flex-col sm:flex-row gap-2 mb-4"
    onsubmit={(event) => {
      event.preventDefault();
      void search();
    }}
  >
    <label class="relative flex-1">
      <span class="sr-only">Title to search</span>
      <span class="absolute left-3 top-1/2 -translate-y-1/2" style="color: var(--paper-450);">
        <Icon name="search" size={14} />
      </span>
      <input
        bind:value={query}
        class="w-full h-9 rounded-xl pl-9 pr-3 text-sm outline-none"
        style="background: var(--input-bg); border: 1px solid var(--line); color: var(--text-1);"
        aria-label="Title to search in sources"
        spellcheck="false"
        data-autofocus
      />
    </label>
    <button type="submit" class="btn-primary px-4 py-2 text-sm shrink-0" disabled={loading}>
      <Icon name="search" size={14} />
      {loading ? "Searching" : "Search sources"}
    </button>
  </form>

  {#if error}
    <Callout tone="danger" class="mb-4">{error}</Callout>
  {/if}

  {#if loading && groups.length === 0}
    <ProgressBar label="Searching installed sources" indeterminate />
  {:else if extensions.length === 0}
    <Callout tone="info" title="No sources installed">
      {#if hiddenNsfw > 0}
        {hiddenNsfw} NSFW {hiddenNsfw === 1 ? "source is" : "sources are"} hidden. Turn on
        Show NSFW sources in Settings to search {hiddenNsfw === 1 ? "it" : "them"}.
      {:else}
        Add a source first, then come back to this title. Kaminari ships no catalog.
      {/if}
      <button class="btn-ghost px-2.5 py-1 text-[11px] mt-2" onclick={onOpenSources}>
        <Icon name="package" size={12} />
        Open Sources
      </button>
    </Callout>
  {:else}
    {#if hiddenNsfw > 0}
      <Callout tone="info" class="mb-4">
        {hiddenNsfw} NSFW {hiddenNsfw === 1 ? "source is" : "sources are"} hidden. Turn on
        Show NSFW sources in Settings to search {hiddenNsfw === 1 ? "it" : "them"}.
      </Callout>
    {/if}
    <div class="space-y-5">
      {#each groups as group (group.extension.id)}
        <section aria-label="{group.extension.name} results">
          <div class="flex flex-wrap items-center gap-2 mb-2">
            <h3 class="text-sm font-semibold">{group.extension.name}</h3>
            <span class="chip text-[10px] px-2 py-0.5">
              {group.extension.kind === "mangayomi" ? "Mangayomi" : "Native"}
            </span>
            {#if !group.loading && !group.error}
              <span class="shelf-count">{group.items.length}</span>
            {/if}
            {#if group.loading}
              <span class="text-[10px]" style="color: var(--paper-450);">Searching</span>
            {/if}
          </div>

          {#if group.loading}
            <ProgressBar label="Searching {group.extension.name}" indeterminate size="sm" />
          {:else if group.error}
            <Callout tone="warning">{group.error}</Callout>
          {:else if group.items.length === 0}
            <p class="text-xs" style="color: var(--paper-450);">
              No matches. Try a shorter title or the original language title.
            </p>
          {:else}
            <div class="space-y-1.5">
              {#each group.items.slice(0, 10) as item (item.id)}
                <button
                  class="w-full flex items-center gap-3 rounded-xl p-2 text-left transition-colors"
                  style="background: var(--well-bg); border: 1px solid var(--line);"
                  onclick={() => onOpen(group.extension, item)}
                  aria-label="Open {item.title} on {group.extension.name}"
                >
                  <span
                    class="w-10 h-14 rounded-lg overflow-hidden shrink-0"
                    style="background: var(--void-800);"
                  >
                    <Artwork src={item.cover} alt="" imgClass="w-full h-full object-cover">
                      {#snippet fallback()}
                        <span class="w-full h-full grid place-items-center text-sm font-bold" style="color: var(--paper-500);">
                          {item.title.slice(0, 1)}
                        </span>
                      {/snippet}
                    </Artwork>
                  </span>
                  <span class="min-w-0 flex-1">
                    <span class="block text-sm font-semibold truncate">{item.title}</span>
                    <span class="block text-[11px] mt-0.5" style="color: var(--paper-450);">
                      {item.year ? item.year : "Open on " + group.extension.name}
                    </span>
                  </span>
                  <Icon name="chevronRight" size={14} class="shrink-0" />
                </button>
              {/each}
            </div>
          {/if}
        </section>
      {/each}
    </div>
  {/if}

  <p class="text-[11px] mt-5" style="color: var(--text-4);">
    Results come from the installed sources. Kaminari does not host or endorse them.
  </p>
</Modal>
