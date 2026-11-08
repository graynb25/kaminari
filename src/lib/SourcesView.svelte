<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import {
    jsExtensionInstallPath,
    jsExtensionInstallSample,
    jsExtensionList,
    jsExtensionRemove,
    jsExtensionTest,
    registryFetch,
    sourceInstall,
    sourceList,
    sourceRemove,
    sourceToggle,
  } from "./api";
  import type {
    Boot,
    InstalledSource,
    JsExtension,
    JsSearchItem,
    LegacyExtension,
  } from "./types";

  let {
    boot,
    notify,
    onBrowse,
  }: {
    boot: Boot | null;
    notify: (text: string, tone?: "info" | "success" | "error") => void;
    onBrowse: (extension: JsExtension) => void;
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
  let jsExtensions = $state<JsExtension[]>([]);
  let jsLoading = $state(true);
  let jsBusy = $state<string | null>(null);
  let jsPath = $state("");
  let jsTestId = $state<string | null>(null);
  let jsTestQuery = $state("Popeye");
  let jsTestLoading = $state(false);
  let jsTestError = $state<string | null>(null);
  let jsTestItems = $state<JsSearchItem[]>([]);

  const mem = $derived(
    boot ? `${Math.round(boot.limits.extMemoryBytes / 1024 / 1024)}MB per runtime` : "Loading",
  );

  const installedIds = $derived(new Set(installed.map((entry) => entry.id)));
  const hostCount = $derived(jsExtensions.reduce((sum, extension) => sum + extension.hosts.length, 0));

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
    void refreshJsExtensions();
  });

  async function refreshJsExtensions() {
    jsLoading = true;
    try {
      jsExtensions = await jsExtensionList();
    } catch {
      jsExtensions = [];
    } finally {
      jsLoading = false;
    }
  }

  async function installSample() {
    jsBusy = "sample";
    try {
      await jsExtensionInstallSample();
      await refreshJsExtensions();
      notify("Installed the public domain sample extension.", "success");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Sample install failed: ${message}`, "error");
    } finally {
      jsBusy = null;
    }
  }

  async function installPath() {
    const path = jsPath.trim();
    if (!path) {
      notify("Paste a path to a .kext package first.", "error");
      return;
    }
    jsBusy = "path";
    try {
      await jsExtensionInstallPath(path);
      jsPath = "";
      await refreshJsExtensions();
      notify("Installed the package as sideloaded.", "success");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Package install failed: ${message}`, "error");
    } finally {
      jsBusy = null;
    }
  }

  async function removeJs(extension: JsExtension) {
    jsBusy = extension.id;
    try {
      await jsExtensionRemove(extension.id);
      if (jsTestId === extension.id) {
        jsTestId = null;
        jsTestItems = [];
        jsTestError = null;
      }
      await refreshJsExtensions();
      notify(`Removed ${extension.name}.`, "success");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Remove failed: ${message}`, "error");
    } finally {
      jsBusy = null;
    }
  }

  async function runJsTest(extension: JsExtension) {
    jsTestId = extension.id;
    jsTestLoading = true;
    jsTestError = null;
    jsTestItems = [];
    try {
      const result = await jsExtensionTest(extension.id, jsTestQuery.trim());
      jsTestItems = result.items ?? [];
      if (jsTestItems.length === 0) {
        jsTestError = "The source answered, but returned no rows.";
      }
    } catch (e) {
      jsTestError = e instanceof Error ? e.message : String(e);
    } finally {
      jsTestLoading = false;
    }
  }

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

  async function toggleSource(entry: InstalledSource) {
    busy = entry.id;
    try {
      await sourceToggle(entry.id, !entry.enabled);
      await refreshInstalled();
      notify(`${entry.name} ${entry.enabled ? "disabled" : "enabled"}.`, "success");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Could not update ${entry.name}: ${message}`, "error");
    } finally {
      busy = null;
    }
  }

  function onFilterInput() {
    limit = 60;
  }

  function shortName(name: string): string {
    return name.replace(/^Aniyomi:\s*/, "");
  }

  function initial(name: string): string {
    return shortName(name).slice(0, 1).toUpperCase();
  }
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <header class="flex flex-wrap items-start gap-4 mb-6">
    <div class="min-w-0">
      <p class="eyebrow">Extensions</p>
      <h1 class="page-title mt-1">Sources</h1>
      <p class="page-sub mt-1 max-w-3xl">
        Kaminari runs its own JavaScript packages in a QuickJS host. The Aniyomi registry below is
        metadata only, because those packages are Android APKs. Neither path bundles a source.
      </p>
    </div>
    <div class="ml-auto flex flex-wrap items-center gap-2">
      <span class="status-pill">
        <span class="status-dot" aria-hidden="true"></span>
        QuickJS host
      </span>
      <span class="status-pill">{mem}</span>
    </div>
  </header>

  <div class="grid grid-cols-2 md:grid-cols-4 gap-3 mb-8">
    <div class="stat-card">
      <p class="stat-label">Native extensions</p>
      <p class="stat-value">{jsExtensions.length}</p>
    </div>
    <div class="stat-card">
      <p class="stat-label">Installed metadata</p>
      <p class="stat-value">{installed.length}</p>
    </div>
    <div class="stat-card">
      <p class="stat-label">Registry packages</p>
      <p class="stat-value">{entries.length}</p>
    </div>
    <div class="stat-card">
      <p class="stat-label">Allowed hosts</p>
      <p class="stat-value">{hostCount}</p>
    </div>
  </div>

  <section class="mb-10" aria-label="Native JavaScript extensions">
    <div class="flex flex-wrap items-baseline gap-3 mb-3">
      <div>
        <p class="eyebrow">QuickJS</p>
        <h2 class="shelf-title mt-1">Native extensions</h2>
      </div>
      <span class="chip text-[10px] px-2 py-0.5">5s calls</span>
      <span class="chip text-[10px] px-2 py-0.5">64MB</span>
      <span class="chip text-[10px] px-2 py-0.5">1MB storage</span>
      <span class="ml-auto text-[10px]" style="color: var(--paper-450); font-family: var(--font-mono);">{mem}</span>
    </div>
    <p class="page-sub mb-4 max-w-3xl">
      Each call gets a fresh runtime. Packages get an HTTPS allowlist, a blocking fetch, an HTML
      selector, SHA-256, namespaced storage and a logger. No filesystem, no child process, no
      environment. The sample reads public domain animation from archive.org.
    </p>

    <div class="surface p-4 mb-4">
      <div class="flex flex-col sm:flex-row gap-2">
        <button
          class="btn-primary px-4 py-2 text-sm shrink-0"
          disabled={jsBusy === "sample"}
          onclick={() => void installSample()}
        >
          <Icon name="download" size={14} />
          {jsBusy === "sample" ? "Installing" : "Install sample"}
        </button>
        <label class="relative flex-1">
          <span class="sr-only">Path to a .kext package</span>
          <span class="absolute left-3 top-1/2 -translate-y-1/2" style="color: var(--paper-450);">
            <Icon name="folder" size={14} />
          </span>
          <input
            bind:value={jsPath}
            placeholder="Path to a .kext package"
            class="w-full h-9 rounded-xl pl-9 pr-3 text-sm outline-none"
            style="background: var(--void-950 / 0.4); border: 1px solid var(--line); color: var(--paper-50); font-family: var(--font-mono);"
            aria-label="Path to a .kext package"
            spellcheck="false"
          />
        </label>
        <button
          class="btn-ghost px-4 py-2 text-sm shrink-0"
          disabled={jsBusy === "path"}
          onclick={() => void installPath()}
        >
          {jsBusy === "path" ? "Installing" : "Install from path"}
        </button>
      </div>
      <p class="text-xs mt-2 leading-relaxed" style="color: var(--paper-450);">
        No registry yet. Every package shows its hosts and trust level. Unsigned packages are
        sideloaded and stay local.
      </p>
    </div>

    {#if jsLoading}
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-3">
        {#each Array(2) as _}
          <div class="h-[160px] rounded-2xl shimmer" aria-hidden="true"></div>
        {/each}
      </div>
    {:else if jsExtensions.length === 0}
      <div class="surface px-5 py-6 flex items-center gap-4">
        <span class="w-10 h-10 grid place-items-center rounded-xl shrink-0" style="background: rgb(108 107 245 / 0.1); border: 1px solid var(--line-volt); color: var(--volt-300);">
          <Icon name="code" size={18} />
        </span>
        <div>
          <p class="text-sm font-semibold">No native extensions installed</p>
          <p class="text-xs mt-0.5" style="color: var(--paper-300);">Install the sample to run a real search through the host.</p>
        </div>
      </div>
    {:else}
      <div class="space-y-3">
        {#each jsExtensions as extension (extension.id)}
          <article class="surface p-4">
            <div class="flex items-start gap-3">
              <div class="source-tile" aria-hidden="true">JS</div>
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-2">
                  <h3 class="text-sm font-semibold">{extension.name}</h3>
                  <span class="chip text-[10px] px-2 py-0.5">v{extension.version}</span>
                  <span class="chip chip-volt text-[10px] px-2 py-0.5">{extension.trust}</span>
                </div>
                <p class="text-[11px] mt-0.5 truncate" style="color: var(--paper-450); font-family: var(--font-mono);" title={extension.id}>
                  {extension.id}
                </p>
                <div class="flex flex-wrap gap-1.5 mt-2">
                  {#each extension.hosts as host}
                    <span class="chip text-[10px] px-2 py-0.5">
                      <Icon name="globe" size={10} />
                      {host}
                    </span>
                  {/each}
                </div>
              </div>
              <div class="flex items-center gap-1.5 shrink-0">
                <button class="btn-primary px-2.5 py-1.5 text-[11px]" onclick={() => onBrowse(extension)}>
                  Browse
                </button>
                <button
                  class="btn-ghost px-2.5 py-1.5 text-[11px]"
                  disabled={jsBusy === extension.id}
                  onclick={() => void removeJs(extension)}
                >
                  Remove
                </button>
              </div>
            </div>

            <form
              class="flex flex-col sm:flex-row gap-2 mt-3"
              onsubmit={(event) => {
                event.preventDefault();
                void runJsTest(extension);
              }}
            >
              <label class="relative flex-1">
                <span class="sr-only">Test search on {extension.name}</span>
                <span class="absolute left-3 top-1/2 -translate-y-1/2" style="color: var(--paper-450);">
                  <Icon name="search" size={13} />
                </span>
                <input
                  bind:value={jsTestQuery}
                  placeholder="Try Popeye or Betty Boop"
                  class="w-full h-8 rounded-lg pl-8 pr-3 text-sm outline-none"
                  style="background: var(--void-950 / 0.4); border: 1px solid var(--line); color: var(--paper-50);"
                  aria-label="Test search on {extension.name}"
                />
              </label>
              <button type="submit" class="btn-primary px-3 py-1.5 text-[11px] shrink-0" disabled={jsTestLoading}>
                {jsTestLoading && jsTestId === extension.id ? "Running" : "Test search"}
              </button>
            </form>

            {#if jsTestId === extension.id}
              <div class="mt-3 rounded-xl p-3" style="background: rgb(5 5 11 / 0.4); border: 1px solid var(--line);" aria-live="polite">
                {#if jsTestLoading}
                  <p class="text-xs flex items-center gap-2" style="color: var(--paper-450);">
                    <span class="w-3 h-3 rounded-full border-2 border-t-transparent animate-spin" style="border-color: var(--volt-400); border-top-color: transparent;"></span>
                    Running a fresh QuickJS runtime.
                  </p>
                {:else if jsTestError}
                  <div class="rounded-lg px-3 py-2 text-xs flex items-start gap-2" style="background: rgb(251 113 133 / 0.1); border: 1px solid rgb(251 113 133 / 0.35); color: var(--paper-50);" role="alert">
                    <Icon name="alert" size={14} class="mt-0.5 shrink-0" />
                    {jsTestError}
                  </div>
                {:else}
                  <div class="space-y-1.5 max-h-[320px] overflow-y-auto pr-1">
                    {#each jsTestItems as item (item.id)}
                      <div class="flex items-center gap-2 px-2 py-1.5 rounded-lg" style="background: rgb(23 23 38 / 0.72); border: 1px solid var(--line);">
                        <div class="w-7 h-10 shrink-0 rounded overflow-hidden" style="background: var(--void-700);">
                          {#if item.cover}
                            <img src={item.cover} alt="" loading="lazy" decoding="async" class="w-full h-full object-cover" />
                          {/if}
                        </div>
                        <p class="text-xs font-semibold flex-1 truncate">{item.title}</p>
                        <span class="text-[10px]" style="color: var(--paper-450); font-family: var(--font-mono);">
                          {item.year ?? ""}
                        </span>
                      </div>
                    {/each}
                  </div>
                  <details class="mt-2">
                    <summary class="text-[11px] cursor-pointer" style="color: var(--paper-450);">Raw JSON</summary>
                    <pre
                      class="mt-1 p-2 rounded-lg text-[10px] overflow-auto max-h-48"
                      style="background: var(--void-950); border: 1px solid var(--line); color: var(--paper-300); font-family: var(--font-mono);">{JSON.stringify(
                        jsTestItems,
                        null,
                        2,
                      )}</pre>
                  </details>
                {/if}
              </div>
            {/if}
          </article>
        {/each}
      </div>
    {/if}
  </section>

  <div class="surface p-4 mb-6">
    <div class="flex items-center gap-3 mb-3">
      <span class="w-8 h-8 grid place-items-center rounded-lg" style="background: rgb(108 107 245 / 0.12); border: 1px solid var(--line-volt); color: var(--volt-300);">
        <Icon name="database" size={16} />
      </span>
      <div class="min-w-0">
        <h2 class="text-sm font-semibold">Registry</h2>
        <p class="text-[11px]" style="color: var(--paper-450);">Aniyomi index, metadata only</p>
      </div>
      <span class="ml-auto text-[10px]" style="color: var(--paper-450); font-family: var(--font-mono);">
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
      <label class="relative flex-1">
        <span class="sr-only">Registry URL</span>
        <span class="absolute left-3 top-1/2 -translate-y-1/2" style="color: var(--paper-450);">
          <Icon name="link" size={14} />
        </span>
        <input
          bind:value={url}
          class="w-full h-9 rounded-xl pl-9 pr-3 text-sm outline-none"
          style="background: var(--void-950 / 0.4); border: 1px solid var(--line); color: var(--paper-50); font-family: var(--font-mono);"
          aria-label="Registry URL"
          spellcheck="false"
        />
      </label>
      <button class="btn-primary px-4 py-2 text-sm shrink-0" type="submit" disabled={loading}>
        <Icon name="refresh" size={14} />
        {loading ? "Reading" : "Load registry"}
      </button>
    </form>
    <p class="text-xs mt-3 leading-relaxed" style="color: var(--paper-450);">
      Legacy entries are unsigned metadata, shown as sideloaded. NSFW and torrent packages carry a
      badge and stay out of the way until you choose them.
    </p>
  </div>

  {#if error}
    <div class="surface px-4 py-3 mb-6 flex flex-col sm:flex-row sm:items-center gap-3 text-sm" role="alert">
      <span class="w-8 h-8 grid place-items-center rounded-lg shrink-0" style="background: rgb(251 113 133 / 0.1); color: var(--danger);">
        <Icon name="alert" size={16} />
      </span>
      <p class="flex-1" style="color: var(--paper-300);">{error}</p>
      <button class="btn-ghost px-3 py-1.5 text-xs shrink-0" onclick={() => void loadRegistry()}>Retry</button>
    </div>
  {/if}

  {#if installed.length > 0}
    <section class="mb-8" aria-label="Installed sources">
      <div class="flex items-baseline gap-3 mb-3">
        <h2 class="shelf-title">Installed</h2>
        <span class="shelf-count">{installed.length}</span>
      </div>
      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
        {#each installed as entry (entry.id)}
          <article class="surface surface-hover p-4 flex items-start gap-3" style="opacity: {entry.enabled ? 1 : 0.62};">
            <div class="source-tile" aria-hidden="true">{initial(entry.name)}</div>
            <div class="min-w-0 flex-1">
              <h3 class="text-sm font-semibold truncate">{shortName(entry.name)}</h3>
              <div class="flex flex-wrap gap-1.5 mt-1.5">
                <span class="chip text-[10px] px-2 py-0.5" style="font-family: var(--font-mono);">v{entry.version}</span>
                <span class="chip text-[10px] px-2 py-0.5">{entry.sourceCount} sources</span>
                {#if entry.nsfw}
                  <span class="chip text-[10px] px-2 py-0.5" style="color: var(--warning); border-color: rgb(251 191 36 / 0.3);">NSFW</span>
                {/if}
                <span class="chip text-[10px] px-2 py-0.5">Metadata</span>
              </div>
            </div>
            <div class="flex flex-col items-end gap-1.5 shrink-0">
              <button
                class="chip text-[10px] px-2 py-1"
                aria-pressed={entry.enabled}
                disabled={busy === entry.id}
                onclick={() => void toggleSource(entry)}
                title={entry.enabled ? `Disable ${shortName(entry.name)}` : `Enable ${shortName(entry.name)}`}
              >
                <span class="status-dot {entry.enabled ? '' : 'is-offline'}" style="width: 4px; height: 4px;" aria-hidden="true"></span>
                {entry.enabled ? "Enabled" : "Disabled"}
              </button>
              <button
                class="btn-ghost px-2.5 py-1.5 text-[11px]"
                disabled={busy === entry.id}
                onclick={() => void remove(entry)}
              >
                Remove
              </button>
            </div>
          </article>
        {/each}
      </div>
    </section>
  {/if}

  <section aria-label="Available packages">
    <div class="flex flex-wrap items-baseline gap-3 mb-3">
      <div>
        <p class="eyebrow">Registry</p>
        <h2 class="shelf-title mt-1">Available</h2>
      </div>
      <span class="shelf-count">
        {entries.length === 0 ? "not loaded" : `${filtered.length} of ${entries.length}`}
      </span>
      <div class="ml-auto flex items-center gap-2">
        <label class="relative block">
          <span class="sr-only">Filter packages</span>
          <span class="absolute left-3 top-1/2 -translate-y-1/2" style="color: var(--paper-450);">
            <Icon name="filter" size={13} />
          </span>
          <input
            bind:value={filter}
            oninput={onFilterInput}
            placeholder="Filter packages"
            class="h-8 w-52 rounded-lg pl-8 pr-3 text-sm outline-none"
            style="background: rgb(23 23 38 / 0.72); border: 1px solid var(--line); color: var(--paper-50);"
            aria-label="Filter packages"
          />
        </label>
        <span class="text-[10px] hidden md:inline" style="color: var(--paper-450); font-family: var(--font-mono);">{mem}</span>
      </div>
    </div>

    {#if loading}
      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
        {#each Array(6) as _}
          <div class="h-[124px] rounded-2xl shimmer" aria-hidden="true"></div>
        {/each}
      </div>
    {:else if entries.length === 0}
      <div class="surface px-5 py-8 text-center">
        <div class="empty-icon mx-auto" aria-hidden="true">
          <Icon name="package" size={26} />
        </div>
        <h3 class="text-lg font-semibold">No registry loaded</h3>
        <p class="text-sm mt-1.5 max-w-md mx-auto" style="color: var(--paper-300);">
          The default is the yuzono Aniyomi index. Press Load registry to inspect it. Nothing is
          installed until you add a package.
        </p>
        <button class="btn-primary mt-4 px-4 py-2 text-sm" onclick={() => void loadRegistry()}>
          <Icon name="download" size={14} />
          Load registry
        </button>
      </div>
    {:else if filtered.length === 0}
      <div class="surface px-5 py-6 text-sm flex items-center gap-3" style="color: var(--paper-300);">
        <span style="color: var(--paper-450);"><Icon name="filter" size={16} /></span>
        <p class="flex-1">No package matches "{filter}". Clear the filter to see all {entries.length}.</p>
        <button class="btn-ghost px-3 py-1.5 text-xs shrink-0" onclick={() => (filter = "")}>Clear</button>
      </div>
    {:else}
      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
        {#each shown as entry (entry.pkg)}
          {@const added = installedIds.has(entry.pkg)}
          {@const name = shortName(entry.name)}
          <article class="surface surface-hover p-4 flex flex-col">
            <div class="flex items-start gap-3">
              <div class="source-tile" aria-hidden="true">{initial(entry.name)}</div>
              <div class="min-w-0 flex-1">
                <h3 class="text-sm font-semibold truncate">{name}</h3>
                <p class="text-[11px] mt-0.5 truncate" style="color: var(--paper-450); font-family: var(--font-mono);" title={entry.pkg}>
                  {entry.pkg}
                </p>
              </div>
              <button
                class="{added ? 'btn-ghost' : 'btn-primary'} px-2.5 py-1.5 text-[11px] shrink-0"
                disabled={busy === entry.pkg || added}
                onclick={() => void add(entry)}
              >
                {added ? "Added" : busy === entry.pkg ? "Adding" : "Add"}
              </button>
            </div>
            <div class="flex flex-wrap gap-1.5 mt-3">
              <span class="chip text-[10px] px-2 py-0.5">v{entry.version}</span>
              <span class="chip text-[10px] px-2 py-0.5">{entry.lang}</span>
              <span class="chip text-[10px] px-2 py-0.5">{entry.sources.length} sources</span>
              {#if entry.nsfw}
                <span class="chip text-[10px] px-2 py-0.5" style="color: var(--warning); border-color: rgb(251 191 36 / 0.3);">NSFW</span>
              {/if}
              {#if entry.torrent}
                <span class="chip text-[10px] px-2 py-0.5">Torrent</span>
              {/if}
            </div>
            <div class="mt-3 pt-3 flex items-center gap-2" style="border-top: 1px solid var(--line);">
              <span class="text-[10px] inline-flex items-center gap-1.5" style="color: var(--paper-450);">
                <Icon name="shield" size={11} />
                Sideloaded metadata
              </span>
              <span class="ml-auto text-[10px]" style="color: var(--paper-450); font-family: var(--font-mono);">
                {entry.sources.length} sources
              </span>
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
