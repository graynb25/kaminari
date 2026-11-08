<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import { playerStatus } from "./api";
  import {
    getThemeMode,
    setThemeMode,
    THEME_MODES,
    type ThemeMode,
  } from "./theme";
  import type { Boot, PlayerStatus } from "./types";

  let {
    boot,
    onOpenShortcuts,
  }: { boot: Boot | null; onOpenShortcuts?: () => void } = $props();

  let player = $state<PlayerStatus | null>(null);
  let themeMode = $state<ThemeMode>(getThemeMode());

  function pickMode(next: ThemeMode) {
    themeMode = next;
    setThemeMode(next);
  }

  onMount(() => {
    void (async () => {
      try {
        player = await playerStatus();
      } catch {
        player = null;
      }
    })();
  });

  // Static section shape. The rows that depend on the async boot snapshot
  // resolve through rowValue below, so the template stays reactive.
  const sections = [
    {
      title: "App",
      icon: "monitor",
      rows: [
        { label: "Version", value: "0.10.2" },
        {
          label: "Stack",
          value: "Tauri 2, Svelte 5 Runes, QuickJS host, in-app player, mpv fallback",
        },
        { label: "Player", value: "" },
      ],
    },
    {
      title: "Data",
      icon: "database",
      rows: [
        { label: "AniList endpoint", value: "" },
        { label: "Library", value: "One local SQLite file, WAL mode, portable backup" },
      ],
    },
    {
      title: "Updates and diagnostics",
      icon: "refresh",
      rows: [
        { label: "Extension budget", value: "" },
        { label: "Auto update", value: "One click restart, lands in P3" },
        { label: "Diagnostics export", value: "Redacted bundle, lands in P3" },
      ],
    },
  ];

  const playerLabel = $derived(
    player === null
      ? "Checking"
      : player.found
        ? player.version ?? "mpv found"
        : "Not found",
  );

  const memoryLabel = $derived(
    boot ? `${Math.round(boot.limits.extMemoryBytes / 1024 / 1024)}MB per runtime` : "Loading",
  );

  function rowValue(label: string, fallback: string): string {
    if (label === "AniList endpoint") return boot?.anilistEndpoint ?? "Loading";
    if (label === "Extension budget") {
      if (!boot) return "Loading";
      const mem = Math.round(boot.limits.extMemoryBytes / 1024 / 1024);
      return `${boot.limits.extTimeoutSecs}s calls, ${boot.limits.extResolveTimeoutSecs}s resolves, ${mem}MB per runtime`;
    }
    if (label === "Player") {
      if (!player) return "Checking";
      if (!player.found) {
        return "Not found. Install mpv, put it on PATH, or set KAMINARI_MPV.";
      }
      return player.version ?? player.path ?? "mpv found";
    }
    return fallback;
  }
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <div class="flex flex-wrap items-start gap-4 mb-6">
    <div>
      <p class="eyebrow">System</p>
      <h1 class="page-title mt-1">Settings</h1>
      <p class="page-sub mt-1 max-w-2xl">
        Controls appear as they ship. Nothing on this page pretends to work before then.
      </p>
    </div>
    {#if onOpenShortcuts}
      <button class="btn-ghost ml-auto px-3.5 py-2 text-xs" onclick={onOpenShortcuts}>
        <Icon name="keyboard" size={14} />
        Keyboard shortcuts
      </button>
    {/if}
  </div>

  <section class="surface appearance-panel p-5 mb-4" aria-labelledby="appearance-title">
    <div class="flex flex-wrap items-start gap-4">
      <div class="flex items-center gap-2.5 min-w-0">
        <span class="w-8 h-8 grid place-items-center rounded-lg" style="background: rgb(108 107 245 / 0.1); border: 1px solid var(--line-volt); color: var(--volt-300);">
          <Icon name="sun" size={16} />
        </span>
        <div class="min-w-0">
          <h2 id="appearance-title" class="text-[15px] font-semibold">Appearance</h2>
          <p class="text-xs mt-0.5" style="color: var(--text-3);">
            Choose how Kaminari looks. System follows Windows.
          </p>
        </div>
      </div>
      <div class="theme-options ml-auto" role="group" aria-label="Theme">
        {#each THEME_MODES as option}
          <button
            class="theme-option"
            class:is-active={themeMode === option.id}
            aria-pressed={themeMode === option.id}
            onclick={() => pickMode(option.id)}
          >
            <span class="theme-option-icon"><Icon name={option.icon} size={15} /></span>
            <span class="min-w-0 text-left">
              <span class="block text-xs font-semibold">{option.label}</span>
              <span class="block text-[10px] mt-0.5" style="color: var(--text-4);">{option.hint}</span>
            </span>
          </button>
        {/each}
      </div>
    </div>
  </section>

  <div class="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-6">
    <div class="stat-card">
      <p class="stat-label">Version</p>
      <p class="stat-value">0.10.2</p>
    </div>
    <div class="stat-card">
      <p class="stat-label">Player</p>
      <div class="flex items-center gap-2 mt-1">
        <span class="status-dot {player && !player.found ? 'is-offline' : ''}" aria-hidden="true"></span>
        <p class="text-sm font-semibold truncate">{playerLabel}</p>
      </div>
    </div>
    <div class="stat-card">
      <p class="stat-label">AniList</p>
      <p class="text-sm font-semibold truncate mt-1" title={boot?.anilistEndpoint ?? "Loading"}>
        {boot?.anilistEndpoint ? "Configured" : "Loading"}
      </p>
    </div>
    <div class="stat-card">
      <p class="stat-label">Runtime budget</p>
      <p class="text-sm font-semibold truncate mt-1">{memoryLabel}</p>
    </div>
  </div>

  <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
    {#each sections as section}
      <section class="surface p-5">
        <div class="flex items-center gap-2.5 mb-3">
          <span class="w-8 h-8 grid place-items-center rounded-lg" style="background: rgb(108 107 245 / 0.1); border: 1px solid var(--line-volt); color: var(--volt-300);">
            <Icon name={section.icon} size={16} />
          </span>
          <h2 class="text-[15px] font-semibold">{section.title}</h2>
        </div>
        <dl>
          {#each section.rows as row, position}
            <div
              class="flex flex-col sm:flex-row sm:items-baseline gap-1 py-2.5"
              style={position > 0 ? "border-top: 1px solid var(--line);" : ""}
            >
              <dt class="text-sm sm:w-40 shrink-0" style="color: var(--paper-450);">{row.label}</dt>
              <dd class="text-sm break-all" style="color: var(--paper-50);">{rowValue(row.label, row.value)}</dd>
            </div>
          {/each}
        </dl>
      </section>
    {/each}
  </div>

  <div class="surface p-5 mt-4 flex flex-wrap items-center gap-4">
    <span class="w-10 h-10 grid place-items-center rounded-xl shrink-0" style="background: rgb(108 107 245 / 0.1); border: 1px solid var(--line-volt); color: var(--volt-300);">
      <Icon name="shield" size={18} />
    </span>
    <div class="min-w-0 flex-1">
      <p class="text-sm font-semibold">Local first</p>
      <p class="text-xs mt-0.5" style="color: var(--paper-400);">
        Library, progress and source runs live in SQLite on this machine. AniList is the only network
        dependency for catalogue data.
      </p>
    </div>
    <span class="chip chip-live text-[10px] px-2.5 py-1">
      <span class="status-dot" style="width: 4px; height: 4px;" aria-hidden="true"></span>
      SQLite
    </span>
  </div>
</div>

<style>
  .appearance-panel {
    background: var(--surface-raised-bg);
  }

  .theme-options {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .theme-option {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 128px;
    padding: 9px 11px;
    border-radius: 13px;
    background: var(--control-bg);
    border: 1px solid var(--line);
    color: var(--text-3);
    transition:
      background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-out);
  }

  .theme-option:hover {
    background: var(--tint-1);
    border-color: var(--line-strong);
    color: var(--text-1);
    transform: translateY(-1px);
  }

  .theme-option.is-active {
    background: linear-gradient(180deg, rgb(139 138 255 / 0.18), rgb(139 138 255 / 0.06));
    border-color: var(--line-volt);
    color: var(--text-1);
    box-shadow: inset 0 0 0 1px rgb(139 138 255 / 0.16);
  }

  .theme-option-icon {
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    border-radius: 9px;
    background: var(--tint-1);
    color: var(--text-3);
  }

  .theme-option.is-active .theme-option-icon {
    background: rgb(139 138 255 / 0.16);
    color: var(--volt-300);
  }
</style>
