<script lang="ts">
  import { onMount } from "svelte";
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

  function onFilterInput() {
    limit = 60;
  }
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <div class="flex items-baseline gap-3 mb-1">
    <h1 class="text-[28px] font-bold" style="font-family: var(--font-display);">Sources</h1>
    <span class="text-sm" style="color: var(--paper-500);">Native host and Aniyomi metadata</span>
  </div>
  <p class="text-sm mb-6 max-w-3xl leading-relaxed" style="color: var(--paper-300);">
    Kaminari runs its own JavaScript packages in a QuickJS host. The Aniyomi registry below is
    metadata only, because those packages are Android APKs. Neither path bundles a source.
  </p>

  <section class="mb-10" aria-label="Native JavaScript extensions">
    <div class="flex flex-wrap items-baseline gap-3 mb-3">
      <h2 class="text-xl font-semibold" style="font-family: var(--font-display);">
        Native extensions
      </h2>
      <span class="text-sm" style="color: var(--paper-500);">QuickJS host</span>
      <span class="text-xs ml-auto" style="color: var(--paper-500); font-family: var(--font-mono);">
        5s calls, 64MB, 1MB storage
      </span>
    </div>
    <p class="text-sm mb-4 max-w-3xl leading-relaxed" style="color: var(--paper-300);">
      Each call gets a fresh runtime. Packages get an HTTPS allowlist, a blocking fetch, an HTML
      selector, SHA-256, namespaced storage and a logger. No filesystem, no child process, no
      environment. The sample reads public domain animation from archive.org.
    </p>

    <div
      class="p-4 rounded-2xl mb-4"
      style="background: var(--void-900); border: 1px solid var(--line);"
    >
      <div class="flex flex-col sm:flex-row gap-2">
        <button
          class="btn-primary px-4 py-2 text-sm shrink-0"
          disabled={jsBusy === "sample"}
          onclick={() => void installSample()}
        >
          {jsBusy === "sample" ? "Installing." : "Install sample"}
        </button>
        <input
          bind:value={jsPath}
          placeholder="Path to a .kext package"
          class="flex-1 h-9 rounded-lg px-3 text-sm outline-none"
          style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-50); font-family: var(--font-mono);"
          aria-label="Path to a .kext package"
          spellcheck="false"
        />
        <button
          class="btn-ghost px-4 py-2 text-sm shrink-0"
          disabled={jsBusy === "path"}
          onclick={() => void installPath()}
        >
          {jsBusy === "path" ? "Installing." : "Install from path"}
        </button>
      </div>
      <p class="text-xs mt-2 leading-relaxed" style="color: var(--paper-500);">
        No registry yet. Every package shows its hosts and trust level. Unsigned packages are
        sideloaded and stay local.
      </p>
    </div>

    {#if jsLoading}
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-3">
        {#each Array(2) as _}
          <div class="h-[132px] rounded-2xl shimmer" aria-hidden="true"></div>
        {/each}
      </div>
    {:else if jsExtensions.length === 0}
      <div
        class="rounded-2xl px-5 py-6 text-sm"
        style="background: var(--void-900); border: 1px dashed var(--line); color: var(--paper-300);"
      >
        No native extensions installed. Install the sample to run a real search through the host.
      </div>
    {:else}
      <div class="space-y-3">
        {#each jsExtensions as extension (extension.id)}
          <article
            class="p-4 rounded-2xl"
            style="background: var(--void-900); border: 1px solid var(--line);"
          >
            <div class="flex items-start gap-3">
              <div
                class="w-10 h-10 shrink-0 grid place-items-center rounded-xl text-xs font-bold"
                style="background: var(--void-800); border: 1px solid var(--line); color: var(--volt-300); font-family: var(--font-mono);"
              >
                JS
              </div>
              <div class="min-w-0 flex-1">
                <h3 class="text-sm font-semibold">{extension.name}</h3>
                <p
                  class="text-[11px] mt-0.5 truncate"
                  style="color: var(--paper-500); font-family: var(--font-mono);"
                  title={extension.id}
                >
                  {extension.id} · v{extension.version} · {extension.trust}
                </p>
                <div class="flex flex-wrap gap-1.5 mt-2">
                  {#each extension.hosts as host}
                    <span class="chip text-[10px] px-2 py-0.5">{host}</span>
                  {/each}
                </div>
              </div>
              <div class="flex items-center gap-1.5 shrink-0">
                <button
                  class="btn-primary px-2.5 py-1 text-xs"
                  onclick={() => onBrowse(extension)}
                >
                  Browse
                </button>
                <button
                  class="btn-ghost px-2.5 py-1 text-xs"
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
              <input
                bind:value={jsTestQuery}
                placeholder="Try Popeye or Betty Boop"
                class="flex-1 h-8 rounded-lg px-3 text-sm outline-none"
                style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-50);"
                aria-label="Test search on {extension.name}"
              />
              <button
                type="submit"
                class="btn-primary px-3 py-1.5 text-xs shrink-0"
                disabled={jsTestLoading}
              >
                {jsTestLoading && jsTestId === extension.id ? "Running." : "Test search"}
              </button>
            </form>

            {#if jsTestId === extension.id}
              <div class="mt-3" aria-live="polite">
                {#if jsTestLoading}
                  <p class="text-xs" style="color: var(--paper-500);">
                    Running a fresh QuickJS runtime.
                  </p>
                {:else if jsTestError}
                  <div
                    class="rounded-lg px-3 py-2 text-xs"
                    style="background: rgb(248 113 113 / 0.1); border: 1px solid rgb(248 113 113 / 0.35); color: var(--paper-50);"
                    role="alert"
                  >
                    {jsTestError}
                  </div>
                {:else}
                  <div class="space-y-1.5 max-h-[320px] overflow-y-auto pr-1">
                    {#each jsTestItems as item (item.id)}
                      <div
                        class="flex items-center gap-2 px-2 py-1.5 rounded-lg"
                        style="background: var(--void-800); border: 1px solid var(--line);"
                      >
                        <div
                          class="w-7 h-10 shrink-0 rounded overflow-hidden"
                          style="background: var(--void-700);"
                        >
                          {#if item.cover}
                            <img
                              src={item.cover}
                              alt=""
                              loading="lazy"
                              class="w-full h-full object-cover"
                            />
                          {/if}
                        </div>
                        <p class="text-xs font-semibold flex-1 truncate">{item.title}</p>
                        <span
                          class="text-[10px]"
                          style="color: var(--paper-500); font-family: var(--font-mono);"
                        >
                          {item.year ?? ""}
                        </span>
                      </div>
                    {/each}
                  </div>
                  <details class="mt-2">
                    <summary class="text-[11px] cursor-pointer" style="color: var(--paper-500);">
                      Raw JSON
                    </summary>
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
