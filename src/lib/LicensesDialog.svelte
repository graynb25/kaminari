<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";

  type LicensePackage = {
    key: string;
    name: string;
    versions: string[];
    license: string;
    source?: string;
  };

  type LicenseGroup = {
    id: string;
    label: string;
    packages: LicensePackage[];
  };

  type LicenseText = {
    license: string;
    text: string;
    source?: string;
  };

  type LicensesData = {
    generatedAt: string;
    appVersion: string;
    platform?: string | null;
    summary: {
      packages: number;
      licenses: number;
      notRecorded: number;
      proprietary: number;
    };
    groups: LicenseGroup[];
    licenseTexts: LicenseText[];
    notes: string[];
  };

  let { onClose }: { onClose: () => void } = $props();

  let data = $state<LicensesData | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let query = $state("");
  let openKey = $state<string | null>(null);
  let dialog = $state<HTMLDivElement | null>(null);
  let searchInput = $state<HTMLInputElement | null>(null);
  let closeButton = $state<HTMLButtonElement | null>(null);
  let previousFocus: HTMLElement | null = null;

  const term = $derived(query.trim().toLowerCase());
  const textByLicense = $derived(
    new Map((data?.licenseTexts ?? []).map((entry) => [entry.license, entry])),
  );
  const filteredGroups = $derived.by(() => {
    if (!data) return [];
    const text = term;
    return data.groups
      .map((group) => ({
        ...group,
        packages: group.packages.filter((pkg) => {
          if (!text) return true;
          return (
            pkg.name.toLowerCase().includes(text) ||
            pkg.license.toLowerCase().includes(text) ||
            (pkg.source ?? "").toLowerCase().includes(text) ||
            pkg.versions.some((version) => version.toLowerCase().includes(text))
          );
        }),
      }))
      .filter((group) => group.packages.length > 0);
  });
  const filteredCount = $derived(
    filteredGroups.reduce((sum, group) => sum + group.packages.length, 0),
  );

  async function load() {
    loading = true;
    error = null;
    try {
      const response = await fetch("/licenses.json", { cache: "no-store" });
      if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
      const parsed = (await response.json()) as LicensesData;
      data = parsed;
    } catch (e) {
      data = null;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function toggle(key: string) {
    openKey = openKey === key ? null : key;
  }

  function trapTab(event: KeyboardEvent) {
    if (event.key !== "Tab" || !dialog) return;
    const items = [...dialog.querySelectorAll<HTMLElement>("button, [href], input, select, textarea, [tabindex='0']")].filter(
      (el) => el.tabIndex >= 0 && el.offsetParent !== null,
    );
    if (items.length === 0) return;
    const first = items[0];
    const last = items[items.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  onMount(() => {
    previousFocus = document.activeElement as HTMLElement | null;
    document.body.dataset.overlay = "licenses";
    void load();
    requestAnimationFrame(() => searchInput?.focus());
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      delete document.body.dataset.overlay;
      previousFocus?.focus?.();
    };
  });
</script>

<div
  class="fixed inset-0 z-[80] flex items-center justify-center p-4 overlay-backdrop overlay-in"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}
  onwheel={(event) => {
    if (event.target === event.currentTarget) event.preventDefault();
  }}
>
  <div
    bind:this={dialog}
    tabindex={-1}
    class="modal-in glass-strong w-full max-w-[980px] max-h-[88vh] rounded-[22px] overflow-hidden flex flex-col"
    role="dialog"
    aria-modal="true"
    aria-label="Licenses"
    onkeydown={trapTab}
  >
    <div class="shrink-0 px-5 py-4" style="border-bottom: 1px solid var(--line);">
      <div class="flex items-center gap-3">
        <span
          class="w-9 h-9 grid place-items-center rounded-xl shrink-0"
          style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);"
        >
          <Icon name="package" size={17} />
        </span>
        <div class="min-w-0 flex-1">
          <h2 class="text-base font-semibold">Licenses</h2>
          <p class="text-[11px]" style="color: var(--text-4);">
            Every package in this build and the license it declares.
          </p>
        </div>
        <button
          bind:this={closeButton}
          class="btn-ghost px-2.5 py-1.5 text-xs"
          onclick={onClose}
          aria-label="Close licenses"
        >
          <Icon name="close" size={14} />
        </button>
      </div>

      <div class="flex flex-wrap items-center gap-2.5 mt-3">
        <label class="license-search flex-1 min-w-[220px] max-w-[520px]">
          <span class="sr-only">Search licenses</span>
          <span class="absolute left-3 top-1/2 -translate-y-1/2" style="color: var(--text-4);">
            <Icon name="search" size={13} />
          </span>
          <input
            bind:this={searchInput}
            bind:value={query}
            placeholder="Search packages, licenses or sources"
            class="w-full h-8 rounded-lg pl-8 pr-3 text-sm outline-none"
            style="background: var(--input-bg); border: 1px solid var(--line); color: var(--text-1);"
            aria-label="Search licenses"
            autocomplete="off"
            spellcheck={false}
          />
        </label>
        {#if data}
          <div class="license-summary">
            <span>{term ? `${filteredCount} of ${data.summary.packages}` : data.summary.packages} packages</span>
            <span class="license-summary-sep" aria-hidden="true"></span>
            <span>{data.summary.licenses} licenses</span>
            <span class="license-summary-sep" aria-hidden="true"></span>
            <span>Generated {data.generatedAt}</span>
          </div>
        {/if}
      </div>
    </div>

    <div class="flex-1 overflow-y-auto px-4 py-4 md:px-5" style="overscroll-behavior: contain;">
      {#if loading && !data}
        <div class="space-y-2" aria-label="Loading licenses">
          {#each Array(6) as _}
            <div class="surface p-3 flex items-center gap-3">
              <div class="shimmer w-9 h-9 rounded-lg shrink-0"></div>
              <div class="flex-1 space-y-2">
                <div class="shimmer h-3 w-1/3 rounded"></div>
                <div class="shimmer h-2 w-1/2 rounded"></div>
              </div>
            </div>
          {/each}
        </div>
      {:else if error && !data}
        <div class="surface p-5 max-w-xl">
          <div class="flex items-start gap-3">
            <span class="w-9 h-9 grid place-items-center rounded-lg shrink-0" style="background: var(--danger-wash); color: var(--danger);">
              <Icon name="alert" size={17} />
            </span>
            <div class="min-w-0 flex-1">
              <h3 class="text-sm font-semibold">Licenses did not load</h3>
              <p class="text-xs mt-1 leading-relaxed" style="color: var(--text-3);">{error}</p>
            </div>
            <button class="btn-ghost px-3 py-1.5 text-xs shrink-0" onclick={() => void load()}>Retry</button>
          </div>
        </div>
      {:else if data}
        {#if data.summary.notRecorded > 0}
          <div class="license-callout" role="status">
            <Icon name="info" size={14} class="mt-0.5 shrink-0" />
            <span>
              {data.summary.notRecorded} {data.summary.notRecorded === 1 ? "package has" : "packages have"} no license recorded. The list says that rather than guessing.
            </span>
          </div>
        {/if}
        {#if data.summary.proprietary > 0}
          <div class="license-callout is-warning" role="status">
            <Icon name="alert" size={14} class="mt-0.5 shrink-0" />
            <span>
              {data.summary.proprietary} {data.summary.proprietary === 1 ? "package is" : "packages are"} not open source. The row names the license.
            </span>
          </div>
        {/if}

        {#if filteredGroups.length === 0}
          <div class="py-12 text-center">
            <div class="empty-icon mx-auto" aria-hidden="true">
              <Icon name="search" size={24} />
            </div>
            <p class="text-sm font-semibold">No packages for "{query.trim()}"</p>
            <p class="text-xs mt-1" style="color: var(--text-4);">Try a shorter name, a license id or a host.</p>
            <button class="btn-ghost px-3 py-1.5 text-xs mt-4" onclick={() => (query = "")}>Clear search</button>
          </div>
        {:else}
          <div class="space-y-4">
            {#each filteredGroups as group (group.id)}
              <section class="surface overflow-hidden" aria-label={group.label}>
                <div class="license-group-head">
                  <h3 class="text-sm font-semibold">{group.label}</h3>
                  <span class="shelf-count">{group.packages.length}</span>
                </div>
                <div>
                  {#each group.packages as pkg (pkg.key)}
                    {@const text = textByLicense.get(pkg.license)}
                    <article class="license-row">
                      <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                        <p class="min-w-0 flex-1 text-sm font-semibold break-all">
                          {pkg.name}
                          <span class="license-versions">{pkg.versions.join(", ")}</span>
                        </p>
                        <span class="license-id" title={pkg.license}>{pkg.license}</span>
                      </div>
                      {#if pkg.source}
                        <p class="license-source" title={pkg.source}>{pkg.source}</p>
                      {/if}
                      <div class="license-actions">
                        {#if text}
                          <button class="license-link" onclick={() => toggle(pkg.key)} aria-expanded={openKey === pkg.key} aria-label="{openKey === pkg.key ? 'Hide' : 'Read'} the {pkg.license} license for {pkg.name}">
                            {openKey === pkg.key ? "Hide license" : "Read license"}
                          </button>
                        {:else}
                          <span class="license-missing">Text not bundled</span>
                        {/if}
                      </div>
                      {#if openKey === pkg.key && text}
                        <div
                          class="license-text-wrap"
                          tabindex="0"
                          role="region"
                          aria-label="{pkg.license} license text for {pkg.name}"
                        >
                          <pre class="license-text">{text.text}</pre>
                        </div>
                      {/if}
                    </article>
                  {/each}
                </div>
              </section>
            {/each}
          </div>
        {/if}

        <div class="license-notes">
          {#if data.platform}
            <p>Rust graph generated for {data.platform}.</p>
          {/if}
          {#each data.notes as note}
            <p>{note}</p>
          {/each}
          <p>Kaminari {data.appVersion}. The list is not a substitute for the license texts themselves.</p>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .license-search {
    position: relative;
  }

  .license-summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-4);
  }

  .license-summary-sep {
    width: 1px;
    height: 12px;
    background: var(--line-strong);
  }

  .license-callout {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-bottom: 12px;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--tint-1);
    border: 1px solid var(--line);
    color: var(--text-2);
    font-size: 11px;
    line-height: 1.55;
  }

  .license-callout.is-warning {
    background: var(--warning-wash);
    border-color: var(--warning-line);
  }

  .license-group-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    background: var(--tint-1);
    border-bottom: 1px solid var(--line);
  }

  .license-row {
    padding: 10px 14px;
    border-top: 1px solid var(--line);
  }

  .license-row:first-child {
    border-top: 0;
  }

  .license-versions {
    margin-left: 4px;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 400;
    color: var(--text-4);
  }

  .license-id {
    max-width: 48%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--chip-bg);
    border: 1px solid var(--line);
    color: var(--text-3);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .license-source {
    margin-top: 3px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-4);
  }

  .license-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 10px;
    margin-top: 5px;
  }

  .license-link {
    font-size: 11px;
    font-weight: 600;
    color: var(--volt-300);
    padding: 2px 0;
    border-bottom: 1px solid transparent;
    transition: border-color var(--dur-fast) var(--ease-out);
  }

  .license-link:hover {
    border-color: var(--line-volt);
  }

  .license-missing {
    font-size: 10px;
    color: var(--text-4);
  }

  .license-text-wrap {
    margin-top: 8px;
    max-height: 320px;
    overflow-y: auto;
    border-radius: 10px;
    background: var(--well-bg);
    border: 1px solid var(--line);
  }

  .license-text {
    margin: 0;
    padding: 12px;
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--text-2);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.6;
  }

  .license-notes {
    margin-top: 18px;
    padding: 0 4px 4px;
    display: grid;
    gap: 4px;
    font-size: 10px;
    line-height: 1.5;
    color: var(--text-4);
  }
</style>
