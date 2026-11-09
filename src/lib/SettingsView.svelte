<script lang="ts">
  import { onMount } from "svelte";
  import BrandMark from "./BrandMark.svelte";
  import Icon from "./Icon.svelte";
  import PageHeader from "./PageHeader.svelte";
  import { playerStatus, proxyStatus } from "./api";
  import { proxyFlagEnabled, setProxyFlag } from "./streamProxy";
  import {
    getThemeMode,
    setThemeMode,
    THEME_MODES,
    type ThemeMode,
  } from "./theme";
  import type { Boot, PlayerStatus, ProxyStatus } from "./types";

  let {
    boot,
    onOpenShortcuts,
    onOpenLicenses,
  }: {
    boot: Boot | null;
    onOpenShortcuts?: () => void;
    onOpenLicenses: () => void;
  } = $props();

  let player = $state<PlayerStatus | null>(null);
  let themeMode = $state<ThemeMode>(getThemeMode());
  let proxy = $state<ProxyStatus | null>(null);
  let proxyOn = $state(proxyFlagEnabled());
  let activeId = $state("settings-appearance");
  let manualActive = false;

  type NavItem = { id: string; title: string; hint: string };

  const navItems: NavItem[] = [
    { id: "settings-appearance", title: "Appearance", hint: "Theme and preview" },
    { id: "settings-app", title: "App", hint: "Version, stack, player" },
    { id: "settings-playback", title: "Playback", hint: "Proxy for protected streams" },
    { id: "settings-data", title: "Data", hint: "Endpoints and local library" },
    { id: "settings-updates-diagnostics", title: "Updates and diagnostics", hint: "Budgets and export" },
    { id: "settings-licenses", title: "Licenses", hint: "Packages and their licenses" },
    { id: "settings-about", title: "About", hint: "Build and credits" },
  ];

  function pickMode(next: ThemeMode) {
    themeMode = next;
    setThemeMode(next);
  }

  function toggleProxy() {
    proxyOn = !proxyOn;
    setProxyFlag(proxyOn);
  }

  function goToSection(id: string) {
    manualActive = true;
    activeId = id;
    const target = document.getElementById(id);
    if (!target) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    target.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "start" });
  }

  onMount(() => {
    void (async () => {
      try {
        player = await playerStatus();
      } catch {
        player = null;
      }
    })();
    void (async () => {
      try {
        proxy = await proxyStatus();
      } catch {
        // No loopback listener in this build. The row shows Unavailable.
        proxy = { port: 0, entries: 0, enabled: false };
      }
    })();

    const scrollRoot = document.getElementById("content");
    const targets: HTMLElement[] = navItems
      .map((item) => document.getElementById(item.id))
      .filter((el): el is HTMLElement => el !== null);
    if (targets.length === 0) return;

    const root = scrollRoot instanceof HTMLElement ? scrollRoot : null;
    let frame = 0;

    const update = () => {
      frame = 0;
      if (manualActive) return;
      const offset = 140;
      const atBottom =
        root !== null && root.scrollTop + root.clientHeight >= root.scrollHeight - 2;
      if (atBottom && navItems.length > 0) {
        activeId = navItems[navItems.length - 1].id;
        return;
      }
      let current = navItems[0]?.id ?? "settings-appearance";
      for (const item of navItems) {
        const el = document.getElementById(item.id);
        if (el && el.getBoundingClientRect().top <= offset) current = item.id;
      }
      activeId = current;
    };

    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(update);
    };

    const onUserScroll = () => {
      manualActive = false;
      schedule();
    };

    const onKeyScroll = (event: KeyboardEvent) => {
      if (
        event.key === "ArrowUp" ||
        event.key === "ArrowDown" ||
        event.key === "PageUp" ||
        event.key === "PageDown" ||
        event.key === "Home" ||
        event.key === "End"
      ) {
        onUserScroll();
      }
    };

    root?.addEventListener("scroll", schedule, { passive: true });
    root?.addEventListener("wheel", onUserScroll, { passive: true });
    root?.addEventListener("touchstart", onUserScroll, { passive: true });
    window.addEventListener("resize", schedule);
    window.addEventListener("keydown", onKeyScroll);
    update();

    return () => {
      root?.removeEventListener("scroll", schedule);
      root?.removeEventListener("wheel", onUserScroll);
      root?.removeEventListener("touchstart", onUserScroll);
      window.removeEventListener("resize", schedule);
      window.removeEventListener("keydown", onKeyScroll);
      if (frame) cancelAnimationFrame(frame);
    };
  });

  // Static section shape. The rows that depend on the async boot snapshot
  // resolve through rowValue below, so the template stays reactive.
  const sections = [
    {
      id: "settings-app",
      title: "App",
      icon: "monitor",
      rows: [
        { label: "Version", value: __APP_VERSION__ },
        {
          label: "Stack",
          value: "Tauri 2, Svelte 5 Runes, QuickJS host, in-app player, mpv fallback",
        },
        { label: "Player", value: "" },
      ],
    },
    {
      id: "settings-playback",
      title: "Playback",
      icon: "play",
      // The proxy toggle and its status render from state, not text rows.
      rows: [],
    },
    {
      id: "settings-data",
      title: "Data",
      icon: "database",
      rows: [
        { label: "AniList endpoint", value: "" },
        { label: "Library", value: "One local SQLite file, WAL mode, portable backup" },
      ],
    },
    {
      id: "settings-updates-diagnostics",
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

  const proxyReady = $derived(proxy !== null && proxy.enabled);

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
  <PageHeader
    eyebrow="System"
    title="Settings"
    subtitle="Controls appear as they ship. Nothing on this page pretends to work before then."
  >
    {#if onOpenShortcuts}
      <button class="btn-ghost px-3.5 py-2 text-xs" onclick={onOpenShortcuts}>
        <Icon name="keyboard" size={14} />
        Keyboard shortcuts
      </button>
    {/if}
  </PageHeader>

  <nav aria-label="Settings sections" class="settings-chips lg:hidden">
    {#each navItems as item}
      <a
        href="#{item.id}"
        class="chip px-3 py-1.5 text-xs"
        class:chip-volt={activeId === item.id}
        aria-current={activeId === item.id ? "location" : undefined}
        onclick={(event) => {
          event.preventDefault();
          goToSection(item.id);
        }}
      >
        {item.title}
      </a>
    {/each}
  </nav>

  <div class="settings-layout">
    <nav aria-label="Settings sections" class="settings-rail hidden lg:block">
      <div class="settings-rail-inner">
        {#each navItems as item}
          <a
            href="#{item.id}"
            class="settings-rail-item"
            aria-current={activeId === item.id ? "location" : undefined}
            onclick={(event) => {
              event.preventDefault();
              goToSection(item.id);
            }}
          >
            <span class="settings-rail-title">{item.title}</span>
            <span class="settings-rail-hint">{item.hint}</span>
          </a>
        {/each}
      </div>
    </nav>

    <div class="settings-content min-w-0">
      <section id="settings-appearance" data-settings-section class="surface appearance-panel settings-section p-5 mb-4" aria-labelledby="appearance-title">
        <div class="flex flex-wrap items-start gap-4">
          <div class="flex items-center gap-2.5 min-w-0">
            <span class="w-8 h-8 grid place-items-center rounded-lg" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
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
        <div class="theme-preview" aria-hidden="true">
          <div class="theme-mini theme-dark">
            <p class="theme-mini-label">Storm</p>
            <div class="theme-mini-swatches">
              <span class="theme-swatch is-a"></span>
              <span class="theme-swatch is-b"></span>
              <span class="theme-swatch is-c"></span>
            </div>
          </div>
          <div class="theme-mini theme-light">
            <p class="theme-mini-label">Daylight</p>
            <div class="theme-mini-swatches">
              <span class="theme-swatch is-c"></span>
              <span class="theme-swatch is-b"></span>
              <span class="theme-swatch is-a"></span>
            </div>
          </div>
        </div>
      </section>

      <div class="grid grid-cols-2 gap-3 mb-6">
        <div class="stat-card">
          <p class="stat-label">Version</p>
          <p class="stat-value">{__APP_VERSION__}</p>
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

      <div class="grid grid-cols-1 xl:grid-cols-2 gap-4">
        {#each sections as section}
          <section id={section.id} data-settings-section class="surface settings-section p-5">
            <div class="flex items-center gap-2.5 mb-3">
              <span class="w-8 h-8 grid place-items-center rounded-lg" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
                <Icon name={section.icon} size={16} />
              </span>
              <h2 class="text-[15px] font-semibold">{section.title}</h2>
            </div>
            {#if section.title === "Playback"}
              <div class="flex flex-wrap items-center justify-between gap-3 py-2.5">
                <div class="min-w-0">
                  <p class="text-sm" style="color: var(--paper-50);">Use loopback proxy for protected streams</p>
                  <p class="text-xs mt-0.5" style="color: var(--paper-400);">
                    Adds request headers and Range handling for MP4 sources. Off by default.
                  </p>
                </div>
                <button
                  class="chip text-[10px] px-2 py-1 shrink-0"
                  aria-label="Use loopback proxy for protected streams"
                  aria-pressed={proxyOn}
                  disabled={!proxyReady}
                  onclick={toggleProxy}
                  title="Use loopback proxy for protected streams"
                >
                  <span class="status-dot {proxyOn ? '' : 'is-offline'}" style="width: 4px; height: 4px;" aria-hidden="true"></span>
                  {proxyOn ? "On" : "Off"}
                </button>
              </div>
              <dl>
                <div class="flex flex-col sm:flex-row sm:items-baseline gap-1 py-2.5" style="border-top: 1px solid var(--line);">
                  <dt class="text-sm sm:w-40 shrink-0" style="color: var(--paper-450);">Loopback proxy</dt>
                  <dd class="text-sm wrap-break-word min-w-0" style="color: var(--paper-50);">
                    {#if proxy === null}
                      Checking
                    {:else if proxy.enabled}
                      <span class="inline-flex flex-wrap items-center gap-2">
                        <span class="mono">127.0.0.1:{proxy.port}</span>
                        <span class="chip chip-live text-[10px] px-2 py-0.5">
                          <span class="status-dot" style="width: 4px; height: 4px;" aria-hidden="true"></span>
                          Ready
                        </span>
                      </span>
                    {:else}
                      Unavailable
                      <span class="block text-xs mt-0.5" style="color: var(--paper-400);">
                        No loopback port is available. Protected streams open in mpv.
                      </span>
                    {/if}
                  </dd>
                </div>
              </dl>
            {:else}
              <dl>
                {#each section.rows as row, position}
                  <div
                    class="flex flex-col sm:flex-row sm:items-baseline gap-1 py-2.5"
                    style={position > 0 ? "border-top: 1px solid var(--line);" : ""}
                  >
                    <dt class="text-sm sm:w-40 shrink-0" style="color: var(--paper-450);">{row.label}</dt>
                    <dd class="text-sm wrap-break-word min-w-0" style="color: var(--paper-50);">{rowValue(row.label, row.value)}</dd>
                  </div>
                {/each}
              </dl>
            {/if}
          </section>
        {/each}
      </div>

      <div class="surface p-5 mt-4 flex flex-wrap items-center gap-4">
        <span class="w-10 h-10 grid place-items-center rounded-xl shrink-0" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
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

      <section id="settings-licenses" data-settings-section class="surface settings-section p-5 mt-4 flex flex-wrap items-center gap-4" aria-label="Licenses">
        <span class="w-10 h-10 grid place-items-center rounded-xl shrink-0" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
          <Icon name="package" size={18} />
        </span>
        <div class="min-w-0 flex-1">
          <p class="text-sm font-semibold">Licenses</p>
          <p class="text-xs mt-0.5" style="color: var(--paper-400);">
            Every Rust crate, npm package, font and built-in sample in this build, with its license and source.
          </p>
        </div>
        <button class="btn-ghost px-3.5 py-2 text-xs" onclick={onOpenLicenses}>
          <Icon name="list" size={14} />
          View licenses
        </button>
      </section>

      <section id="settings-about" data-settings-section class="surface settings-section p-5 mt-4 flex flex-wrap items-center gap-4" aria-label="About Kaminari">
        <BrandMark size={48} />
        <div class="min-w-0 flex-1">
          <p class="text-sm font-semibold">Kaminari {__APP_VERSION__}</p>
          <p class="text-xs mt-0.5" style="color: var(--paper-400);">
            Anime desktop. Tauri 2, Svelte 5 Runes, QuickJS host, local SQLite library.
          </p>
        </div>
      </section>
    </div>
  </div>
</div>

<style>
  .appearance-panel {
    background: var(--surface-raised-bg);
  }

  .settings-section {
    scroll-margin-top: 84px;
  }

  .settings-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 20px;
  }

  @media (min-width: 1024px) {
    .settings-chips {
      display: none;
    }
  }

  .settings-layout {
    display: block;
  }

  @media (min-width: 1024px) {
    .settings-layout {
      display: grid;
      grid-template-columns: 232px minmax(0, 1fr);
      gap: 24px;
      align-items: start;
    }
  }

  .settings-rail-inner {
    position: sticky;
    top: 20px;
    display: grid;
    gap: 6px;
    max-height: calc(100vh - 120px);
    overflow-y: auto;
    padding: 6px;
    border-radius: var(--radius-md);
    background: var(--surface-card);
    border: 1px solid var(--line);
  }

  .settings-rail-item {
    display: block;
    padding: 9px 11px;
    border-radius: 11px;
    border: 1px solid transparent;
    color: var(--text-2);
    text-decoration: none;
    transition:
      background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .settings-rail-item:hover {
    background: var(--tint-1);
    color: var(--text-1);
  }

  .settings-rail-item[aria-current="location"] {
    background: linear-gradient(180deg, var(--accent-wash-strong), var(--accent-wash-faint));
    border-color: var(--line-volt);
    color: var(--text-1);
  }

  .settings-rail-title {
    display: block;
    font-size: 13px;
    font-weight: 650;
    line-height: 1.3;
  }

  .settings-rail-hint {
    display: block;
    margin-top: 2px;
    font-size: 11px;
    line-height: 1.45;
    color: var(--text-4);
  }

  .settings-rail-item[aria-current="location"] .settings-rail-hint {
    color: var(--text-3);
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
    background: linear-gradient(180deg, var(--accent-wash-strong), var(--accent-wash-faint));
    border-color: var(--line-volt);
    color: var(--text-1);
    box-shadow: inset 0 0 0 1px var(--accent-wash-strong);
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
    background: var(--accent-wash-strong);
    color: var(--volt-300);
  }

  .theme-preview {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 16px;
  }

  .theme-mini {
    flex: 0 1 210px;
    width: 210px;
    max-width: 100%;
    min-width: 0;
    padding: 10px 11px 11px;
    border-radius: 13px;
    background: var(--void-900);
    border: 1px solid var(--line);
  }

  .theme-mini-label {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-4);
  }

  .theme-mini-swatches {
    display: flex;
    gap: 6px;
    margin-top: 8px;
  }

  .theme-swatch {
    flex: 0 0 auto;
    width: 42px;
    aspect-ratio: 2 / 3;
    border-radius: 6px;
    border: 1px solid var(--line);
  }

  .theme-swatch.is-a {
    background:
      radial-gradient(circle at 30% 20%, var(--accent-wash-strong), transparent 60%),
      var(--void-800);
  }

  .theme-swatch.is-b {
    background: var(--void-700);
  }

  .theme-swatch.is-c {
    background:
      linear-gradient(180deg, var(--accent-wash-strong), var(--accent-wash-faint));
  }
</style>
