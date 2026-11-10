<script lang="ts">
  import { onMount } from "svelte";
  import Callout from "./Callout.svelte";
  import Icon from "./Icon.svelte";
  import PageHeader from "./PageHeader.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import SourcePreferencesDialog from "./SourcePreferencesDialog.svelte";
  import {
    jsExtensionInstallPackage,
    jsExtensionInstallPick,
    jsExtensionInstallSample,
    jsExtensionList,
    jsExtensionRemove,
    jsExtensionTest,
    mangayomiInstall,
    mangayomiPreview,
  } from "./api";
  import { filterNsfwSources, showNsfwEnabled } from "./nsfw";
  import type {
    Boot,
    JsExtension,
    JsSearchItem,
    MangayomiPreview,
    MangayomiSourcePreview,
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

  let jsExtensions = $state<JsExtension[]>([]);
  let jsLoading = $state(true);
  let jsBusy = $state<string | null>(null);
  let prefsExtension = $state<JsExtension | null>(null);
  let jsTestId = $state<string | null>(null);
  let jsTestQuery = $state("Popeye");
  let jsTestLoading = $state(false);
  let jsTestError = $state<string | null>(null);
  let jsTestItems = $state<JsSearchItem[]>([]);
  let mangayomiUrl = $state("");
  let mangayomiPreviewState = $state<MangayomiPreview | null>(null);
  let mangayomiError = $state<string | null>(null);
  let mangayomiBusy = $state<string | null>(null);

  const mem = $derived(
    boot ? `${Math.round(boot.limits.extMemoryBytes / 1024 / 1024)}MB per runtime` : "Loading",
  );
  const memShort = $derived(
    boot ? `${Math.round(boot.limits.extMemoryBytes / 1024 / 1024)}MB` : "Loading",
  );
  const showNsfw = showNsfwEnabled();
  const visibleExtensions = $derived(filterNsfwSources(jsExtensions, showNsfw));
  const hiddenNsfwCount = $derived(jsExtensions.length - visibleExtensions.length);
  const visiblePreviewSources = $derived(
    mangayomiPreviewState
      ? filterNsfwSources(mangayomiPreviewState.sources, showNsfw)
      : [],
  );
  const hiddenPreviewCount = $derived(
    mangayomiPreviewState
      ? mangayomiPreviewState.sources.length - visiblePreviewSources.length
      : 0,
  );
  const nativeCount = $derived(
    visibleExtensions.filter((extension) => extension.kind !== "mangayomi").length,
  );
  const mangayomiCount = $derived(
    visibleExtensions.filter((extension) => extension.kind === "mangayomi").length,
  );
  const hostCount = $derived(
    visibleExtensions.reduce((sum, extension) => sum + extension.hosts.length, 0),
  );

  onMount(() => {
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
    jsBusy = "path";
    try {
      const token = await jsExtensionInstallPick();
      if (!token) return;
      await jsExtensionInstallPackage(token);
      await refreshJsExtensions();
      notify("Installed the package as sideloaded.", "success");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Package install failed: ${message}`, "error");
    } finally {
      jsBusy = null;
    }
  }

  async function inspectMangayomi() {
    const target = mangayomiUrl.trim();
    if (!target) {
      notify("Paste a raw JavaScript source URL first.", "error");
      return;
    }
    mangayomiBusy = "inspect";
    mangayomiError = null;
    mangayomiPreviewState = null;
    try {
      const preview = await mangayomiPreview(target);
      mangayomiPreviewState = preview;
      if (preview.sources.length === 0) {
        mangayomiError = "The file has no anime sources.";
      }
    } catch (e) {
      mangayomiError = e instanceof Error ? e.message : String(e);
    } finally {
      mangayomiBusy = null;
    }
  }

  async function installMangayomi(source: MangayomiSourcePreview) {
    if (!mangayomiPreviewState) return;
    mangayomiBusy = `install-${source.index}`;
    mangayomiError = null;
    try {
      await mangayomiInstall(
        mangayomiPreviewState.url,
        source.index,
        mangayomiPreviewState.sha256,
      );
      mangayomiPreviewState = null;
      mangayomiUrl = "";
      await refreshJsExtensions();
      notify(`Installed ${source.name}. It is a third party source.`, "success");
    } catch (e) {
      mangayomiError = e instanceof Error ? e.message : String(e);
    } finally {
      mangayomiBusy = null;
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

</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <PageHeader
    eyebrow="Extensions"
    title="Sources"
    subtitle="Kaminari runs native .kext packages and third-party Mangayomi sources in a QuickJS host. It ships no source list; you add a source you trust."
  >
    <span class="status-pill">
      <span class="status-dot" aria-hidden="true"></span>
      QuickJS host
    </span>
    <span class="status-pill">{mem}</span>
  </PageHeader>

  <div class="grid grid-cols-2 md:grid-cols-4 gap-3 mb-8">
    <div class="stat-card">
      <p class="stat-label">Native extensions</p>
      <p class="stat-value">{nativeCount}</p>
    </div>
    <div class="stat-card">
      <p class="stat-label">Mangayomi sources</p>
      <p class="stat-value">{mangayomiCount}</p>
    </div>
    <div class="stat-card">
      <p class="stat-label">Allowed hosts</p>
      <p class="stat-value">{hostCount}</p>
    </div>
    <div class="stat-card">
      <p class="stat-label">Runtime memory</p>
      <p class="stat-value">{memShort}</p>
    </div>
  </div>

  <section class="mb-10" aria-label="Native JavaScript extensions">
    <div class="flex flex-wrap items-baseline gap-3 mb-3">
      <div>
        <p class="eyebrow">QuickJS</p>
        <h2 class="shelf-title mt-1">Native extensions</h2>
      </div>
      <span class="chip text-[10px] px-2 py-0.5">5s native calls</span>
      <span class="chip text-[10px] px-2 py-0.5">60s source calls</span>
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
        <button
          class="btn-ghost px-4 py-2 text-sm shrink-0"
          disabled={jsBusy === "path"}
          onclick={() => void installPath()}
        >
          <Icon name="folder" size={14} />
          {jsBusy === "path" ? "Installing" : "Choose .kext package"}
        </button>
      </div>
      <p class="text-xs mt-2 leading-relaxed" style="color: var(--paper-450);">
        Every package shows its hosts and trust level. Unsigned packages are sideloaded and stay
        local.
      </p>
      {#if jsBusy}
        <ProgressBar
          label={jsBusy === "sample"
            ? "Installing sample extension"
            : jsBusy === "path"
              ? "Installing package"
              : "Updating extension"}
          indeterminate
          class="mt-3"
        />
      {/if}
    </div>

    {#if jsLoading}
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-3">
        {#each Array(2) as _}
          <div class="h-[160px] rounded-2xl shimmer" aria-hidden="true"></div>
        {/each}
      </div>
    {:else if jsExtensions.length === 0}
      <div class="surface px-5 py-6 flex items-center gap-4">
        <span class="w-10 h-10 grid place-items-center rounded-xl shrink-0" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
          <Icon name="code" size={18} />
        </span>
        <div>
          <p class="text-sm font-semibold">No native extensions installed</p>
          <p class="text-xs mt-0.5" style="color: var(--paper-300);">Install the sample to run a real search through the host.</p>
        </div>
      </div>
    {:else if visibleExtensions.length === 0}
      <div class="surface px-5 py-6 flex items-center gap-4">
        <span class="w-10 h-10 grid place-items-center rounded-xl shrink-0" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
          <Icon name="eyeOff" size={18} />
        </span>
        <div>
          <p class="text-sm font-semibold">Installed sources are hidden</p>
          <p class="text-xs mt-0.5" style="color: var(--paper-300);">
            {hiddenNsfwCount} {hiddenNsfwCount === 1 ? "source is" : "sources are"} marked NSFW.
            Turn on Show NSFW sources in Settings to see {hiddenNsfwCount === 1 ? "it" : "them"}.
          </p>
        </div>
      </div>
    {:else}
      {#if hiddenNsfwCount > 0}
        <Callout tone="info" class="mb-3">
          {hiddenNsfwCount} NSFW {hiddenNsfwCount === 1 ? "source is" : "sources are"} hidden.
          Turn on Show NSFW sources in Settings to show {hiddenNsfwCount === 1 ? "it" : "them"}.
        </Callout>
      {/if}
      <div class="space-y-3">
        {#each visibleExtensions as extension (extension.id)}
          <article class="surface p-4">
            <div class="flex items-start gap-3">
              <div class="source-tile" aria-hidden="true">JS</div>
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-2">
                  <h3 class="text-sm font-semibold">{extension.name}</h3>
                  <span class="chip text-[10px] px-2 py-0.5">v{extension.version}</span>
                  {#if extension.kind === "mangayomi"}
                    <span class="chip chip-volt text-[10px] px-2 py-0.5">Mangayomi</span>
                  {:else}
                    <span class="chip text-[10px] px-2 py-0.5">Native</span>
                  {/if}
                  <span class="chip chip-volt text-[10px] px-2 py-0.5">{extension.trust}</span>
                  {#if extension.nsfw}
                    <span class="chip chip-volt text-[10px] px-2 py-0.5">NSFW</span>
                  {/if}
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
                {#if extension.kind === "mangayomi"}
                  <button
                    class="btn-ghost px-2.5 py-1.5 text-[11px]"
                    onclick={() => (prefsExtension = extension)}
                  >
                    Settings
                  </button>
                {/if}
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
                  style="background: var(--input-bg); border: 1px solid var(--line); color: var(--text-1);"
                  aria-label="Test search on {extension.name}"
                />
              </label>
              <button type="submit" class="btn-primary px-3 py-1.5 text-[11px] shrink-0" disabled={jsTestLoading}>
                {jsTestLoading && jsTestId === extension.id ? "Running" : "Test search"}
              </button>
            </form>

            {#if jsTestId === extension.id}
              <div class="mt-3 rounded-xl p-3" style="background: var(--well-bg); border: 1px solid var(--line);" aria-live="polite">
                {#if jsTestLoading}
                  <div class="space-y-2">
                    <p class="text-xs flex items-center gap-2" style="color: var(--paper-450);">
                      <span class="w-3 h-3 rounded-full border-2 border-t-transparent animate-spin" style="border-color: var(--volt-400); border-top-color: transparent;"></span>
                      Running a fresh QuickJS runtime.
                    </p>
                    <ProgressBar label="Running extension test" indeterminate />
                  </div>
                {:else if jsTestError}
                  <Callout tone="danger">{jsTestError}</Callout>
                {:else}
                  <div class="space-y-1.5 max-h-[320px] overflow-y-auto pr-1">
                    {#each jsTestItems as item (item.id)}
                      <div class="flex items-center gap-2 px-2 py-1.5 rounded-lg" style="background: var(--control-bg); border: 1px solid var(--line);">
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
                      style="background: var(--well-bg); border: 1px solid var(--line); color: var(--text-2); font-family: var(--font-mono);">{JSON.stringify(
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

  <section class="mb-8" aria-label="Mangayomi sources">
    <div class="flex flex-wrap items-baseline gap-3 mb-3">
      <div>
        <p class="eyebrow">Third party</p>
        <h2 class="shelf-title mt-1">Mangayomi sources</h2>
      </div>
      <span class="chip text-[10px] px-2 py-0.5">JavaScript</span>
      <span class="chip text-[10px] px-2 py-0.5">Anime only</span>
    </div>
    <p class="page-sub mb-4 max-w-3xl">
      Paste a raw Mangayomi JavaScript source URL, inspect it, then install it. The source is
      stored locally and runs in the same sandbox as a native package. Kaminari does not include
      a source list, endorse a source, or solve a site's browser challenge.
    </p>

    <div class="surface p-4">
      <form
        class="flex flex-col sm:flex-row gap-2"
        onsubmit={(event) => {
          event.preventDefault();
          void inspectMangayomi();
        }}
      >
        <label class="relative flex-1">
          <span class="sr-only">Raw Mangayomi source URL</span>
          <span class="absolute left-3 top-1/2 -translate-y-1/2" style="color: var(--paper-450);">
            <Icon name="link" size={14} />
          </span>
          <input
            bind:value={mangayomiUrl}
            placeholder="https://example.com/source.js"
            class="w-full h-9 rounded-xl pl-9 pr-3 text-sm outline-none"
            style="background: var(--input-bg); border: 1px solid var(--line); color: var(--text-1); font-family: var(--font-mono);"
            aria-label="Raw Mangayomi source URL"
            spellcheck="false"
          />
        </label>
        <button
          type="submit"
          class="btn-primary px-4 py-2 text-sm shrink-0"
          disabled={mangayomiBusy === "inspect"}
        >
          <Icon name="search" size={14} />
          {mangayomiBusy === "inspect" ? "Inspecting" : "Inspect source"}
        </button>
      </form>

      {#if mangayomiBusy === "inspect"}
        <ProgressBar label="Downloading and inspecting source" indeterminate class="mt-3" />
      {/if}

      {#if mangayomiError}
        <Callout tone="danger" class="mt-3">{mangayomiError}</Callout>
      {/if}

      {#if mangayomiPreviewState}
        <Callout tone="warning" title="Third party code" class="mt-3">
          This source can contact any host and send it what it reads. Kaminari does not review
          it. Install only sources you trust.
        </Callout>
        <p class="text-[11px] mt-3" style="color: var(--paper-450); font-family: var(--font-mono);">
          sha256 {mangayomiPreviewState.sha256.slice(0, 16)}... · {Math.ceil(
            mangayomiPreviewState.bytes / 1024,
          )} KB
        </p>
        {#if hiddenPreviewCount > 0}
          <Callout tone="info" class="mt-3">
            {hiddenPreviewCount} NSFW {hiddenPreviewCount === 1 ? "source is" : "sources are"} hidden.
            Turn on Show NSFW sources in Settings to inspect {hiddenPreviewCount === 1 ? "it" : "them"}.
          </Callout>
        {/if}
        {#if visiblePreviewSources.length === 0}
          <p class="text-xs mt-3" style="color: var(--paper-450);">
            Every source in this package is marked NSFW and hidden by your setting.
          </p>
        {:else}
          <div class="space-y-2 mt-3">
            {#each visiblePreviewSources as source (source.index)}
            <div
              class="rounded-xl p-3 flex flex-col sm:flex-row sm:items-center gap-3"
              style="background: var(--well-bg); border: 1px solid var(--line);"
            >
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-2">
                  <p class="text-sm font-semibold">{source.name}</p>
                  <span class="chip text-[10px] px-2 py-0.5">v{source.version || "unknown"}</span>
                  <span class="chip text-[10px] px-2 py-0.5">{source.lang || "all"}</span>
                  {#if source.nsfw}
                    <span class="chip chip-volt text-[10px] px-2 py-0.5">NSFW</span>
                  {/if}
                </div>
                <p class="text-[11px] mt-0.5 truncate" style="color: var(--paper-450); font-family: var(--font-mono);" title={source.baseUrl}>
                  {source.baseUrl}
                </p>
                {#if source.sourceCodeUrl}
                  <p class="text-[10px] mt-0.5 truncate" style="color: var(--paper-450); font-family: var(--font-mono);" title={source.sourceCodeUrl}>
                    {source.sourceCodeUrl}
                  </p>
                {/if}
              </div>
              <button
                class="btn-primary px-3 py-1.5 text-[11px] shrink-0"
                disabled={mangayomiBusy === `install-${source.index}`}
                onclick={() => void installMangayomi(source)}
              >
                {mangayomiBusy === `install-${source.index}` ? "Installing" : "Install"}
              </button>
            </div>
          {/each}
          </div>
        {/if}
      {/if}
    </div>
  </section>

  {#if prefsExtension}
    <SourcePreferencesDialog
      extension={{ id: prefsExtension.id, name: prefsExtension.name }}
      onClose={() => (prefsExtension = null)}
      {notify}
    />
  {/if}
</div>
