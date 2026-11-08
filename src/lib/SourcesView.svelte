<script lang="ts">
  import { onMount } from "svelte";
  import { registryFetch, sourceInstall, sourceList, sourceRemove } from "./api";
  import type { Boot, InstalledSource, LegacyExtension } from "./types";

  let {
    boot,
    notify,
  }: {
    boot: Boot | null;
    notify: (text: string, tone?: "info" | "success" | "error") => void;
  } = $props();

  const DEFAULT_REGISTRY =
    "https://raw.githubusercontent.com/yuzono/anime-repo/repo/index.min.json";

  let url = $state(DEFAULT_REGISTRY);
  let entries = $state<LegacyExtension[]>([]);
  let installed = $state<InstalledSource[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let filter = $state("");
  let busy = $state<string | null>(null);
  let limit = $state(60);

  const mem = $derived(
    boot ? `${Math.round(boot.limits.extMemoryBytes / 1024 / 1024)}MB per runtime` : "Loading",
  );

  const installedIds = $derived(new Set(installed.map((entry) => entry.id)));

  const filtered = $derived.by(() => {
    const query = filter.trim().toLowerCase();
    if (!query) return entries;
    return entries.filter(
      (entry) =>
        entry.name.toLowerCase().includes(query) ||
        entry.pkg.toLowerCase().includes(query) ||
        entry.lang.toLowerCase().includes(query),
    );
  });

  const shown = $derived(filtered.slice(0, limit));

  onMount(() => {
    void refreshInstalled();
  });

  async function refreshInstalled() {
    try {
      installed = await sourceList();
    } catch {
      installed = [];
    }
  }

  async function loadRegistry() {
    const target = url.trim();
    if (!target) {
      error = "Paste a registry URL first.";
      return;
    }
    loading = true;
    error = null;
    try {
      entries = await registryFetch(target);
      limit = 60;
      if (entries.length === 0) {
        error = "The registry answered but no valid packages were found.";
      } else {
        notify(`Loaded ${entries.length} packages from the registry.`, "success");
      }
    } catch (e) {
      entries = [];
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function add(entry: LegacyExtension) {
    busy = entry.pkg;
    try {
      await sourceInstall(entry);
      await refreshInstalled();
      notify(`Added ${entry.name}. Metadata only until the host runs.`, "success");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Could not add ${entry.name}: ${message}`, "error");
    } finally {
      busy = null;
    }
  }

  async function remove(entry: InstalledSource) {
    busy = entry.id;
    try {
      await sourceRemove(entry.id);
      await refreshInstalled();
      notify(`Removed ${entry.name}.`, "success");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Could not remove ${entry.name}: ${message}`, "error");
    } finally {
      busy = null;
    }
  }

  function onFilterInput() {
    limit = 60;
  }
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <div class="flex items-baseline gap-3 mb-1">
    <h1 class="text-[28px] font-bold" style="font-family: var(--font-display);">Sources</h1>
    <span class="text-sm" style="color: var(--paper-500);">Aniyomi registry, metadata only</span>
  </div>
  <p class="text-sm mb-6 max-w-3xl leading-relaxed" style="color: var(--paper-300);">
    The Aniyomi ecosystem publishes a signed catalogue as JSON. Paste a registry URL, inspect what
    each package covers, and add it to your catalog. Kaminari does not download APKs or run package
    code yet. The QuickJS host lands in P1.
  </p>

  <div
    class="p-4 rounded-2xl mb-6"
    style="background: var(--void-900); border: 1px solid var(--line);"
  >
    <div class="flex items-center gap-2 mb-3">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--volt-400)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <ellipse cx="12" cy="5.5" rx="7.5" ry="2.8" />
        <path d="M4.5 5.5v13c0 1.6 3.4 2.8 7.5 2.8s7.5-1.2 7.5-2.8v-13" />
        <path d="M4.5 12c0 1.6 3.4 2.8 7.5 2.8s7.5-1.2 7.5-2.8" />
      </svg>
      <h2 class="text-[15px] font-semibold">Registry</h2>
      <span class="text-xs" style="color: var(--paper-500); font-family: var(--font-mono);">
        {installed.length} installed
      </span>
    </div>
    <form
      class="flex flex-col sm:flex-row gap-2"
      onsubmit={(event) => {
        event.preventDefault();
        void loadRegistry();
      }}
    >
      <input
        bind:value={url}
        class="flex-1 h-9 rounded-lg px-3 text-sm outline-none"
        style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-50); font-family: var(--font-mono);"
        aria-label="Registry URL"
        spellcheck="false"
      />
      <button class="btn-primary px-4 py-2 text-sm shrink-0" type="submit" disabled={loading}>
        {loading ? "Reading…" : "Load registry"}
      </button>
    </form>
    <p class="text-xs mt-3 leading-relaxed" style="color: var(--paper-500);">
      Legacy entries are unsigned metadata, shown as sideloaded. NSFW and torrent packages carry a
      badge and stay out of the way until you choose them.
    </p>
  </div>

  {#if error}
    <div
      class="rounded-xl px-4 py-3 mb-6 flex flex-col sm:flex-row sm:items-center gap-2 text-sm"
      style="background: var(--void-800); border: 1px solid var(--line);"
      role="alert"
    >
      <p class="flex-1" style="color: var(--paper-300);">{error}</p>
      <button class="btn-ghost px-3 py-1.5 text-xs shrink-0" onclick={() => void loadRegistry()}>
        Retry
      </button>
    </div>
  {/if}

  {#if installed.length > 0}
    <section class="mb-8" aria-label="Installed sources">
      <div class="flex items-baseline gap-3 mb-3">
        <h2 class="text-xl font-semibold" style="font-family: var(--font-display);">Installed</h2>
        <span class="text-sm" style="color: var(--paper-500);">{installed.length}</span>
      </div>
      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
        {#each installed as entry (entry.id)}
          <article
            class="p-4 rounded-2xl flex items-start gap-3"
            style="background: var(--void-900); border: 1px solid var(--line);"
          >
            <div
              class="w-10 h-10 shrink-0 grid place-items-center rounded-xl text-sm font-bold"
              style="background: var(--void-800); border: 1px solid var(--line); color: var(--volt-300); font-family: var(--font-display);"
            >
              {entry.name.replace(/^Aniyomi:\s*/, "").slice(0, 1)}
            </div>
            <div class="min-w-0 flex-1">
              <h3 class="text-sm font-semibold truncate">{entry.name.replace(/^Aniyomi:\s*/, "")}</h3>
              <p class="text-[11px] mt-0.5" style="color: var(--paper-500); font-family: var(--font-mono);">
                v{entry.version} · {entry.sourceCount} sources
              </p>
              {#if entry.nsfw}
                <span class="chip text-[10px] px-1.5 py-0.5 mt-2 inline-block" style="color: var(--warning, #fbbf24);">NSFW</span>
              {/if}
            </div>
            <button
              class="btn-ghost px-2.5 py-1 text-xs shrink-0"
              disabled={busy === entry.id}
              onclick={() => void remove(entry)}
            >
              Remove
            </button>
          </article>
        {/each}
      </div>
    </section>
  {/if}

  <section aria-label="Available packages">
    <div class="flex flex-wrap items-baseline gap-3 mb-3">
      <h2 class="text-xl font-semibold" style="font-family: var(--font-display);">Available</h2>
      <span class="text-sm" style="color: var(--paper-500);">
        {entries.length === 0 ? "not loaded" : `${filtered.length} of ${entries.length}`}
      </span>
      <div class="ml-auto flex items-center gap-2">
        <input
          bind:value={filter}
          oninput={onFilterInput}
          placeholder="Filter packages"
          class="h-8 w-52 rounded-lg px-3 text-sm outline-none"
          style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-50);"
          aria-label="Filter packages"
        />
        <span class="text-[11px]" style="color: var(--paper-500); font-family: var(--font-mono);">
          {mem}
        </span>
      </div>
    </div>

    {#if loading}
      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
        {#each Array(6) as _}
          <div class="h-[112px] rounded-2xl shimmer" aria-hidden="true"></div>
        {/each}
      </div>
    {:else if entries.length === 0}
      <div
        class="rounded-2xl px-5 py-8 text-center"
        style="background: var(--void-900); border: 1px dashed var(--line);"
      >
        <p class="text-4xl mb-2" aria-hidden="true">◇</p>
        <h3 class="text-lg font-semibold">No registry loaded</h3>
        <p class="text-sm mt-1.5 max-w-md mx-auto" style="color: var(--paper-300);">
          The default is the yuzono Aniyomi index. Press Load registry to inspect it. Nothing is
          installed until you add a package.
        </p>
        <button class="btn-primary mt-4 px-4 py-2 text-sm" onclick={() => void loadRegistry()}>
          Load registry
        </button>
      </div>
    {:else if filtered.length === 0}
      <div
        class="rounded-2xl px-5 py-6 text-sm"
        style="background: var(--void-900); border: 1px solid var(--line); color: var(--paper-300);"
      >
        No package matches "{filter}". Clear the filter to see all {entries.length}.
      </div>
    {:else}
      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
        {#each shown as entry (entry.pkg)}
          {@const added = installedIds.has(entry.pkg)}
          {@const shortName = entry.name.replace(/^Aniyomi:\s*/, "")}
          <article
            class="p-4 rounded-2xl flex flex-col"
            style="background: var(--void-900); border: 1px solid var(--line);"
          >
            <div class="flex items-start gap-3">
              <div
                class="w-10 h-10 shrink-0 grid place-items-center rounded-xl text-sm font-bold"
                style="background: var(--void-800); border: 1px solid var(--line); color: var(--volt-300); font-family: var(--font-display);"
              >
                {shortName.slice(0, 1)}
              </div>
              <div class="min-w-0 flex-1">
                <h3 class="text-sm font-semibold truncate">{shortName}</h3>
                <p class="text-[11px] mt-0.5 truncate" style="color: var(--paper-500); font-family: var(--font-mono);" title={entry.pkg}>
                  {entry.pkg}
                </p>
              </div>
              <button
                class="{added ? 'btn-ghost' : 'btn-primary'} px-2 py-1 text-xs shrink-0"
                disabled={busy === entry.pkg || added}
                onclick={() => void add(entry)}
              >
                {added ? "Added" : busy === entry.pkg ? "Adding…" : "Add"}
              </button>
            </div>
            <div class="flex flex-wrap gap-1.5 mt-3">
              <span class="chip text-[10px] px-2 py-0.5">v{entry.version}</span>
              <span class="chip text-[10px] px-2 py-0.5">{entry.lang}</span>
              <span class="chip text-[10px] px-2 py-0.5">{entry.sources.length} sources</span>
              {#if entry.nsfw}
                <span class="chip text-[10px] px-2 py-0.5" style="color: var(--warning, #fbbf24);">NSFW</span>
              {/if}
              {#if entry.torrent}
                <span class="chip text-[10px] px-2 py-0.5">Torrent</span>
              {/if}
            </div>
          </article>
        {/each}
      </div>
      {#if filtered.length > shown.length}
        <div class="flex justify-center mt-5">
          <button class="btn-ghost px-4 py-2 text-sm" onclick={() => (limit += 60)}>
            Show 60 more
          </button>
        </div>
      {/if}
    {/if}
  </section>
</div>
