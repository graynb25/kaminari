<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { confirm, open, save } from "@tauri-apps/plugin-dialog";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import BrandMark from "./BrandMark.svelte";
  import Icon from "./Icon.svelte";
  import PageHeader from "./PageHeader.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import { playerStatus, proxyStatus } from "./api";
  import {
    anilistAuthLogout,
    anilistAuthStart,
    anilistAuthStatus,
    onAnilistAuthChanged,
    type AniListAuthStatus,
  } from "./anilistAuth";
  import { anilistSyncPending, anilistSyncPush } from "./anilistSync";
  import { readDownloadsFolder, saveDownloadsFolder } from "./downloads";
  import {
    readQualityPreference,
    saveQualityPreference,
    type QualityPreference,
  } from "./quality";
  import QualitySelect from "./QualitySelect.svelte";
  import { proxyFlagEnabled, setProxyFlag } from "./streamProxy";
  import { applySettings, collectSettings, parseSettings } from "./settingsBackup";
  import {
    getThemeMode,
    setThemeMode,
    THEME_MODES,
    type ThemeMode,
  } from "./theme";
  import { ACCENTS, getAccent, setAccent, type AccentId } from "./accent";
  import { autoplayEnabled, setAutoplayEnabled } from "./autoplay";
  import { setShowNsfwEnabled, showNsfwEnabled } from "./nsfw";
  import {
    downloadLabel,
    downloadPercent,
    formatReleaseDate,
    initialDownloadProgress,
    missingReleaseFeed,
    releaseNotesExcerpt,
    trackDownload,
    updateInstallMessage,
    type DownloadProgress,
  } from "./updater";
  import type { Boot, PlayerStatus, ProxyStatus } from "./types";

  let {
    boot,
    incognito = false,
    onToggleIncognito,
    onOpenShortcuts,
    onOpenLicenses,
    onOpenOnboarding,
    onDataChanged,
  }: {
    boot: Boot | null;
    incognito?: boolean;
    onToggleIncognito?: (enabled: boolean) => void;
    onOpenShortcuts?: () => void;
    onOpenLicenses: () => void;
    onOpenOnboarding?: () => void;
    onDataChanged?: () => void;
  } = $props();

  let player = $state<PlayerStatus | null>(null);
  let themeMode = $state<ThemeMode>(getThemeMode());
  let accent = $state<AccentId>(getAccent());
  let proxy = $state<ProxyStatus | null>(null);
  let proxyOn = $state(proxyFlagEnabled());
  let activeId = $state("settings-appearance");
  let manualActive = false;
  let dataBusy = $state(false);
  let downloadsFolder = $state("");
  let qualityPreference = $state<QualityPreference>("auto");
  let autoplayOn = $state(true);
  let showNsfw = $state(showNsfwEnabled());
  let folderBusy = $state(false);
  let dataStatus = $state("");
  let dataStatusTone = $state<"success" | "error">("success");
  let restartNeeded = $state(false);
  let anilistAuth = $state<AniListAuthStatus | null>(null);
  let anilistBusy = $state(false);
  let anilistWaiting = $state(false);
  let anilistSyncing = $state(false);
  let anilistPushing = $state(false);
  let anilistPending = $state(0);
  let anilistError = $state("");
  let anilistNotice = $state("");

  type UpdatePhase =
    | "idle"
    | "checking"
    | "current"
    | "available"
    | "downloading"
    | "installed"
    | "error";
  type ReleaseInfo = { version: string; date?: string; body?: string };

  const isWindows = typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);
  let updatePhase = $state<UpdatePhase>("idle");
  let updateMessage = $state("Not checked yet. Updates use the signed release feed.");
  let updateInfo = $state<ReleaseInfo | null>(null);
  let updateSoftError = $state(false);
  let download = $state<DownloadProgress>(initialDownloadProgress());
  let updaterDisposed = false;
  let activeUpdate: Update | null = null;

  const updateBusy = $derived(
    updatePhase === "checking" || updatePhase === "downloading",
  );
  const updateTone = $derived(
    updatePhase === "error"
      ? updateSoftError
        ? "warning"
        : "error"
      : updatePhase === "current" || updatePhase === "installed"
        ? "success"
        : "muted",
  );
  const updateStatus = $derived(
    updatePhase === "downloading" ? downloadLabel(download) : updateMessage,
  );
  const updatePercent = $derived(downloadPercent(download));
  const updateNotes = $derived(updateInfo ? releaseNotesExcerpt(updateInfo.body) : "");

  const anilistHasError = $derived(Boolean(anilistError || anilistAuth?.error));
  const anilistStatusText = $derived(
    anilistError ||
      (anilistWaiting
        ? "Waiting for AniList approval in your browser."
        : anilistNotice || anilistAuth?.error || ""),
  );
  const anilistStatusError = $derived(
    Boolean(
      anilistError || (!anilistWaiting && !anilistNotice && anilistAuth?.error),
    ),
  );

  type NavItem = { id: string; title: string; hint: string };

  const navItems: NavItem[] = [
    { id: "settings-appearance", title: "Appearance", hint: "Theme and preview" },
    { id: "settings-app", title: "App", hint: "Version, stack, player" },
    { id: "settings-playback", title: "Playback", hint: "Proxy for protected streams" },
    { id: "settings-privacy", title: "Privacy", hint: "Incognito and NSFW sources" },
    { id: "settings-data", title: "Data", hint: "Endpoints and local library" },
    { id: "settings-updates-diagnostics", title: "Updates and diagnostics", hint: "Version, budgets, export" },
    { id: "settings-licenses", title: "Licenses", hint: "Packages and their licenses" },
    { id: "settings-about", title: "About", hint: "Build and credits" },
  ];

  function pickMode(next: ThemeMode) {
    themeMode = next;
    setThemeMode(next);
  }

  function pickAccent(next: AccentId) {
    accent = next;
    setAccent(next);
  }

  function toggleProxy() {
    proxyOn = !proxyOn;
    setProxyFlag(proxyOn);
  }

  function backupFileName(): string {
    const now = new Date();
    const pad = (value: number) => String(value).padStart(2, "0");
    return `kaminari-backup-${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}.db`;
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function setDataStatus(tone: "success" | "error", text: string, restart = false) {
    dataStatusTone = tone;
    dataStatus = text;
    restartNeeded = restart;
  }

  function resetDataStatus() {
    dataStatus = "";
    restartNeeded = false;
  }

  function errorText(err: unknown): string {
    if (typeof err === "string") return err;
    if (err instanceof Error) return err.message;
    return "File operation failed.";
  }

  function pickedPath(value: string | string[] | null): string | null {
    if (typeof value === "string") return value;
    if (Array.isArray(value)) return value[0] ?? null;
    return null;
  }

  function failureText(err: unknown): string {
    if (typeof err === "string") return err;
    if (err instanceof Error) return err.message;
    return "Unknown error.";
  }

  async function closeActiveUpdate() {
    const previous = activeUpdate;
    activeUpdate = null;
    if (!previous) return;
    try {
      await previous.close();
    } catch {
      // The plugin releases the resource when the app closes.
    }
  }

  async function checkForUpdates(options: { quiet?: boolean } = {}) {
    if (updateBusy) return;
    updateSoftError = Boolean(options.quiet);
    updatePhase = "checking";
    updateMessage = "Checking the release feed.";
    updateInfo = null;
    download = initialDownloadProgress();
    await closeActiveUpdate();
    try {
      const found = await check();
      if (updaterDisposed) {
        if (found) await found.close().catch(() => {});
        return;
      }
      if (!found) {
        updatePhase = "current";
        updateMessage = "You are up to date.";
        return;
      }
      activeUpdate = found;
      updateInfo = { version: found.version, date: found.date, body: found.body };
      updatePhase = "available";
      updateMessage = `Kaminari ${found.version} is available.`;
    } catch (err) {
      if (updaterDisposed) return;
      const absentFeed = missingReleaseFeed(err);
      updatePhase = "error";
      updateSoftError = Boolean(options.quiet) || absentFeed;
      updateMessage = absentFeed
        ? "No public release feed is published for this build. Updates stay local until a signed release exists."
        : options.quiet
          ? "Could not reach the release feed. This is expected until a signed release is published. The rest of Kaminari works normally."
          : `Update check failed. ${failureText(err)}`;
    }
  }

  async function downloadUpdate() {
    const update = activeUpdate;
    if (!update || updateBusy) return;
    const version = updateInfo?.version ?? update.version;
    updateSoftError = false;
    updatePhase = "downloading";
    updateMessage = `Downloading Kaminari ${version}.`;
    download = initialDownloadProgress();
    try {
      await update.downloadAndInstall(
        (event) => {
          download = trackDownload(download, event);
          if (event.event === "Finished" && !updaterDisposed) {
            updatePhase = "installed";
            updateMessage = updateInstallMessage(version, isWindows);
          }
        },
        { restartAfterInstall: isWindows },
      );
      if (updaterDisposed) return;
      updatePhase = "installed";
      updateMessage = updateInstallMessage(version, isWindows);
    } catch (err) {
      if (updaterDisposed) return;
      updatePhase = "error";
      updateMessage = `Update download failed. ${failureText(err)}`;
    }
  }

  async function restartForUpdate() {
    try {
      await invoke("app_restart");
    } catch (err) {
      updatePhase = "error";
      updateSoftError = false;
      updateMessage = `Restart failed. ${failureText(err)}`;
    }
  }

  async function backupLibrary() {
    if (dataBusy) return;
    resetDataStatus();
    dataBusy = true;
    try {
      const path = pickedPath(
        await save({
          title: "Backup library",
          defaultPath: backupFileName(),
          filters: [{ name: "Kaminari backup", extensions: ["db"] }],
        }),
      );
      if (!path) return;
      const result = await invoke<{ path: string; bytes: number }>("backup_export", { path });
      setDataStatus("success", `Backup saved (${formatBytes(result.bytes)}).`);
    } catch (err) {
      setDataStatus("error", errorText(err));
    } finally {
      dataBusy = false;
    }
  }

  async function changeDownloadsFolder() {
    if (folderBusy) return;
    resetDataStatus();
    folderBusy = true;
    try {
      const path = pickedPath(
        await open({
          title: "Choose downloads folder",
          directory: true,
          multiple: false,
        }),
      );
      if (!path) return;
      saveDownloadsFolder(path);
      downloadsFolder = path;
      setDataStatus("success", "Downloads folder saved.");
    } catch (err) {
      setDataStatus("error", errorText(err));
    } finally {
      folderBusy = false;
    }
  }

  async function restoreBackup() {
    if (dataBusy) return;
    resetDataStatus();
    dataBusy = true;
    try {
      const path = pickedPath(
        await open({
          title: "Restore backup",
          multiple: false,
          directory: false,
          filters: [{ name: "Kaminari backup", extensions: ["db"] }],
        }),
      );
      if (!path) return;
      const approved = await confirm(
        "Replace the current library with this backup? Restart Kaminari after the restore.",
        { title: "Restore backup", kind: "warning" },
      );
      if (!approved) return;
      await invoke<void>("backup_restore", { path });
      setDataStatus("success", "Backup restored. Restart to load it.", true);
    } catch (err) {
      setDataStatus("error", errorText(err));
    } finally {
      dataBusy = false;
    }
  }

  async function exportSettings() {
    if (dataBusy) return;
    resetDataStatus();
    dataBusy = true;
    try {
      const path = pickedPath(
        await save({
          title: "Export settings",
          defaultPath: "kaminari-settings.json",
          filters: [{ name: "Kaminari settings", extensions: ["json"] }],
        }),
      );
      if (!path) return;
      const json = JSON.stringify(collectSettings(), null, 2);
      await invoke<void>("settings_export", { path, json });
      setDataStatus("success", "Settings exported.");
    } catch (err) {
      setDataStatus("error", errorText(err));
    } finally {
      dataBusy = false;
    }
  }

  async function importSettings() {
    if (dataBusy) return;
    resetDataStatus();
    dataBusy = true;
    try {
      const path = pickedPath(
        await open({
          title: "Import settings",
          multiple: false,
          directory: false,
          filters: [{ name: "Kaminari settings", extensions: ["json"] }],
        }),
      );
      if (!path) return;
      const raw = await invoke<string>("settings_read", { path });
      const data = parseSettings(raw);
      applySettings(data);
      themeMode = data.theme;
      accent = data.accent;
      proxyOn = data.proxy;
      setDataStatus("success", "Settings imported.");
    } catch (err) {
      setDataStatus("error", errorText(err));
    } finally {
      dataBusy = false;
    }
  }

  async function restartNow() {
    try {
      await invoke("app_restart");
    } catch (err) {
      setDataStatus("error", errorText(err));
    }
  }

  async function connectAniList() {
    if (anilistBusy || anilistWaiting) return;
    anilistError = "";
    anilistNotice = "";
    if (anilistAuth?.error) {
      anilistAuth = { ...anilistAuth, error: null };
    }
    anilistBusy = true;
    anilistWaiting = true;
    try {
      await anilistAuthStart();
    } catch (err) {
      anilistWaiting = false;
      anilistError = errorText(err);
    } finally {
      anilistBusy = false;
    }
  }

  async function disconnectAniList() {
    if (anilistBusy || anilistSyncing) return;
    anilistBusy = true;
    anilistError = "";
    anilistNotice = "";
    try {
      await anilistAuthLogout();
      anilistAuth = { connected: false, viewer: null, error: null };
      anilistWaiting = false;
    } catch (err) {
      anilistError = errorText(err);
    } finally {
      anilistBusy = false;
    }
  }

  async function syncAniList() {
    if (anilistBusy || anilistSyncing) return;
    anilistError = "";
    anilistNotice = "";
    if (anilistAuth?.error) {
      anilistAuth = { ...anilistAuth, error: null };
    }
    anilistSyncing = true;
    try {
      const result = await invoke<{ titles: number; viewerName: string }>(
        "anilist_sync_pull",
      );
      anilistNotice = `Synced ${result.titles} titles from AniList.`;
      await refreshAniListPending();
      onDataChanged?.();
    } catch (err) {
      anilistError = errorText(err);
    } finally {
      anilistSyncing = false;
    }
  }

  async function refreshAniListPending() {
    try {
      anilistPending = await anilistSyncPending();
    } catch {
      // The count is informative. A failure here should not block the account UI.
    }
  }

  async function pushAniListChanges() {
    if (anilistPushing) return;
    anilistError = "";
    anilistNotice = "";
    if (anilistAuth?.error) {
      anilistAuth = { ...anilistAuth, error: null };
    }
    anilistPushing = true;
    try {
      const result = await anilistSyncPush();
      anilistPending = result.pending;
      if (result.conflicts > 0) {
        anilistNotice =
          result.conflicts === 1
            ? `Sent ${result.pushed} changes. AniList had a newer change for 1 title, so the AniList value was kept.`
            : `Sent ${result.pushed} changes. AniList had newer changes for ${result.conflicts} titles, so the AniList values were kept.`;
      } else if (result.pushed > 0) {
        anilistNotice = `Sent ${result.pushed} changes to AniList.`;
      } else {
        anilistNotice = "AniList is already up to date.";
      }
      onDataChanged?.();
    } catch (err) {
      anilistError = errorText(err);
      await refreshAniListPending();
    } finally {
      anilistPushing = false;
    }
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
    downloadsFolder = readDownloadsFolder() ?? "";
    qualityPreference = readQualityPreference();
    autoplayOn = autoplayEnabled();
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

  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    let eventSeq = 0;

    void refreshAniListPending();

    void (async () => {
      try {
        const stop = await onAnilistAuthChanged((status) => {
          if (disposed) return;
          eventSeq += 1;
          anilistAuth = status;
          anilistWaiting = false;
          anilistError = "";
          anilistNotice = "";
        });
        if (disposed) stop();
        else unlisten = stop;
      } catch (err) {
        if (!disposed) anilistError = errorText(err);
      }

      try {
        const seq = eventSeq;
        const status = await anilistAuthStatus();
        if (!disposed && seq === eventSeq) anilistAuth = status;
      } catch (err) {
        if (!disposed) anilistError = errorText(err);
      }
    })();

    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  onMount(() => {
    updaterDisposed = false;
    void checkForUpdates({ quiet: true });
    return () => {
      updaterDisposed = true;
      if (updatePhase !== "downloading") void closeActiveUpdate();
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
      id: "settings-privacy",
      title: "Privacy",
      icon: "eyeOff",
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
        {
          label: "Diagnostics export",
          value: "Copy redacted diagnostics from error screens. No bundle export yet.",
        },
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
        <div class="accent-row">
          <div class="min-w-0">
            <p class="text-xs font-semibold">Accent</p>
            <p class="text-[10px] mt-0.5" style="color: var(--text-4);">
              Presets keep the contrast. Violet is the signature.
            </p>
          </div>
          <div class="accent-options" role="group" aria-label="Accent color">
            {#each ACCENTS as option}
              <button
                class="accent-option"
                class:is-active={accent === option.id}
                aria-pressed={accent === option.id}
                onclick={() => pickAccent(option.id)}
                title={option.label}
              >
                <span class="accent-swatch" style="--swatch: {option.swatch};"></span>
                {option.label}
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
              <div
                class="flex flex-wrap items-center justify-between gap-3 py-2.5"
                style="border-top: 1px solid var(--line);"
              >
                <div class="min-w-0">
                  <p class="text-sm" style="color: var(--paper-50);">Autoplay next episode</p>
                  <p class="text-xs mt-0.5" style="color: var(--paper-400);">
                    Starts the next episode when the current one ends. Off shows the end card only.
                  </p>
                </div>
                <button
                  class="chip text-[10px] px-2 py-1 shrink-0"
                  aria-label="Autoplay next episode"
                  aria-pressed={autoplayOn}
                  onclick={() => {
                    autoplayOn = !autoplayOn;
                    setAutoplayEnabled(autoplayOn);
                  }}
                  title="Autoplay next episode"
                >
                  <span class="status-dot {autoplayOn ? '' : 'is-offline'}" style="width: 4px; height: 4px;" aria-hidden="true"></span>
                  {autoplayOn ? "On" : "Off"}
                </button>
              </div>
              <div
                class="flex flex-wrap items-center justify-between gap-3 py-2.5"
                style="border-top: 1px solid var(--line);"
              >
                <div class="min-w-0">
                  <p class="text-sm" style="color: var(--paper-50);">Preferred playback quality</p>
                  <p class="text-xs mt-0.5" style="color: var(--paper-400);">
                    Picks the closest stream when a source offers several. Auto keeps source order.
                  </p>
                </div>
                <QualitySelect
                  value={qualityPreference}
                  onChange={(value) => {
                    qualityPreference = value;
                    saveQualityPreference(value);
                  }}
                  label="Preferred playback quality"
                />
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
            {:else if section.title === "Privacy"}
              <div class="flex flex-wrap items-center justify-between gap-3 py-2.5">
                <div class="min-w-0">
                  <p class="text-sm" style="color: var(--paper-50);">Incognito mode</p>
                  <p class="text-xs mt-0.5" style="color: var(--paper-400);">
                    Playback is not written to history, progress, Continue Watching or AniList.
                    Library changes still work.
                  </p>
                </div>
                <button
                  class="chip text-[10px] px-2 py-1 shrink-0"
                  aria-label="Incognito mode"
                  aria-pressed={incognito}
                  disabled={!onToggleIncognito}
                  onclick={() => onToggleIncognito?.(!incognito)}
                  title="Incognito mode"
                >
                  <span class="status-dot {incognito ? '' : 'is-offline'}" style="width: 4px; height: 4px;" aria-hidden="true"></span>
                  {incognito ? "On" : "Off"}
                </button>
              </div>
              <p class="text-xs mt-2 leading-relaxed" style="color: var(--paper-450);">
                Incognito is local to this device. Turn it off here when you want playback to count
                again. The player shows an Incognito badge while it is active.
              </p>
              <div
                class="flex flex-wrap items-center justify-between gap-3 py-2.5 mt-3"
                style="border-top: 1px solid var(--line);"
              >
                <div class="min-w-0">
                  <p class="text-sm" style="color: var(--paper-50);">Show NSFW sources</p>
                  <p class="text-xs mt-0.5" style="color: var(--paper-400);">
                    Off by default. NSFW sources stay installed but are hidden from the Sources page
                    and from source search while this is off.
                  </p>
                </div>
                <button
                  class="chip text-[10px] px-2 py-1 shrink-0"
                  aria-label="Show NSFW sources"
                  aria-pressed={showNsfw}
                  onclick={() => {
                    showNsfw = !showNsfw;
                    setShowNsfwEnabled(showNsfw);
                  }}
                  title="Show NSFW sources"
                >
                  <span class="status-dot {showNsfw ? '' : 'is-offline'}" style="width: 4px; height: 4px;" aria-hidden="true"></span>
                  {showNsfw ? "On" : "Off"}
                </button>
              </div>
              <div class="mt-4 pt-3" style="border-top: 1px solid var(--line);">
                <p class="text-sm" style="color: var(--paper-50);">History and Continue Watching</p>
                <p class="text-xs mt-0.5" style="color: var(--paper-400);">
                  Clear the activity list from the History page. That also empties Continue Watching
                  but keeps your library and watched marks.
                </p>
              </div>
            {:else}
              {#if section.id === "settings-updates-diagnostics"}
                <div class="update-block">
                  <div class="flex flex-wrap items-center gap-3">
                    <div class="min-w-0 flex-1">
                      <p class="text-sm" style="color: var(--paper-50);">Current version</p>
                      <p class="update-version mt-0.5">{__APP_VERSION__}</p>
                    </div>
                    <button
                      class="btn-ghost px-3 py-2 text-xs shrink-0"
                      disabled={updateBusy}
                      onclick={() => void checkForUpdates()}
                    >
                      <Icon name="refresh" size={14} />
                      {updatePhase === "checking" ? "Checking" : "Check for updates"}
                    </button>
                  </div>

                  <div
                    class="update-status"
                    class:is-success={updateTone === "success"}
                    class:is-warning={updateTone === "warning"}
                    class:is-error={updateTone === "error"}
                    role="status"
                    aria-live="polite"
                  >
                    <p class="update-message">{updateStatus}</p>

                    {#if updatePhase === "available" && updateInfo}
                      <div class="update-release">
                        <div class="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
                          <p class="text-sm font-semibold" style="color: var(--paper-50);">
                            Kaminari {updateInfo.version}
                          </p>
                          {#if updateInfo.date}
                            <p class="update-meta">Published {formatReleaseDate(updateInfo.date)}</p>
                          {/if}
                        </div>
                        {#if updateNotes}
                          <p class="update-notes">{updateNotes}</p>
                        {/if}
                        <button
                          class="btn-ghost px-3 py-2 text-xs mt-2.5"
                          disabled={updateBusy}
                          onclick={() => void downloadUpdate()}
                        >
                          <Icon name="download" size={14} />
                          Download and install
                        </button>
                      </div>
                    {/if}

                    {#if updatePhase === "downloading"}
                      <div class="update-progress">
                        <ProgressBar
                          value={updatePercent}
                          label="Update download progress"
                          indeterminate={updatePercent === null}
                          size="sm"
                        />
                      </div>
                    {/if}

                    {#if updatePhase === "installed"}
                      {#if isWindows}
                        <p class="text-xs mt-2.5" style="color: var(--paper-450);">
                          The installer is running. Kaminari will reopen when it finishes.
                        </p>
                      {:else}
                        <button
                          class="btn-primary px-3 py-2 text-xs mt-2.5"
                          disabled={updateBusy}
                          onclick={() => void restartForUpdate()}
                        >
                          <Icon name="refresh" size={14} />
                          Restart to apply
                        </button>
                      {/if}
                    {/if}
                  </div>

                  <p class="update-note">
                    Updates are signed and checked before install. The release feed is live when a signed release is published on GitHub.
                  </p>
                </div>
              {/if}
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
              {#if section.id === "settings-data"}
                <div class="flex flex-wrap items-center gap-3 py-3" style="border-top: 1px solid var(--line);">
                  <span class="w-8 h-8 grid place-items-center rounded-lg shrink-0" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
                    <Icon name="folder" size={15} />
                  </span>
                  <div class="min-w-0 flex-1">
                    <p class="text-sm" style="color: var(--paper-50);">Downloads folder</p>
                    <p class="text-xs mt-0.5 break-all" style="color: var(--paper-400); font-family: var(--font-mono);">
                      {downloadsFolder || "Not set. Kaminari asks when you start a download."}
                    </p>
                  </div>
                  <button
                    class="btn-ghost px-3 py-2 text-xs"
                    disabled={folderBusy}
                    onclick={changeDownloadsFolder}
                  >
                    <Icon name="folder" size={14} />
                    {downloadsFolder ? "Change" : "Choose"}
                  </button>
                </div>
                <div class="anilist-account" style="border-top: 1px solid var(--line);">
                  <div class="flex flex-wrap items-center gap-3 pt-3">
                    {#if anilistAuth?.viewer}
                      {#if anilistAuth.viewer.avatar}
                        <img
                          class="anilist-avatar"
                          src={anilistAuth.viewer.avatar}
                          alt=""
                          loading="lazy"
                        />
                      {:else}
                        <span class="w-9 h-9 grid place-items-center rounded-full shrink-0" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
                          <Icon name="eye" size={16} />
                        </span>
                      {/if}
                      <div class="min-w-0 flex-1">
                        <p class="text-sm font-semibold truncate">{anilistAuth.viewer.name}</p>
                        <p class="text-xs mt-0.5" style="color: var(--paper-400);">
                          AniList account connected
                          <span class="chip text-[10px] px-2 py-0.5 ml-1.5">{anilistPending} pending</span>
                        </p>
                      </div>
                      <button
                        class="btn-ghost px-3 py-2 text-xs"
                        disabled={anilistPushing || anilistSyncing}
                        onclick={pushAniListChanges}
                      >
                        <Icon name="arrowUpRight" size={14} />
                        {anilistPushing ? "Sending" : "Sync now"}
                      </button>
                      <button
                        class="btn-ghost px-3 py-2 text-xs"
                        disabled={anilistBusy || anilistSyncing || anilistPushing}
                        onclick={syncAniList}
                      >
                        <Icon name="refresh" size={14} />
                        {anilistSyncing ? "Syncing" : "Sync from AniList"}
                      </button>
                      <button
                        class="btn-ghost px-3 py-2 text-xs"
                        disabled={anilistBusy || anilistSyncing || anilistPushing}
                        onclick={disconnectAniList}
                      >
                        <Icon name="xCircle" size={14} />
                        Disconnect
                      </button>
                    {:else}
                      <span class="w-9 h-9 grid place-items-center rounded-full shrink-0" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
                        <Icon name="link" size={16} />
                      </span>
                      <div class="min-w-0 flex-1">
                        <p class="text-sm font-semibold">AniList account</p>
                        <p class="text-xs mt-0.5" style="color: var(--paper-400);">
                          Connect to keep your AniList list and progress in step.
                        </p>
                      </div>
                      <button
                        class="btn-ghost px-3 py-2 text-xs"
                        disabled={anilistBusy || anilistWaiting}
                        onclick={connectAniList}
                      >
                        <Icon name="external" size={14} />
                        {anilistHasError ? "Reconnect" : "Connect AniList"}
                      </button>
                    {/if}
                  </div>
                  {#if anilistStatusText}
                    <p
                      role="status"
                      class="data-status"
                      class:is-error={anilistStatusError}
                    >
                      {anilistStatusText}
                    </p>
                  {/if}
                </div>
                <div class="data-actions" style="border-top: 1px solid var(--line);">
                  <div class="flex flex-wrap items-center gap-2 pt-3">
                    <button
                      class="btn-ghost px-3 py-2 text-xs"
                      disabled={dataBusy}
                      onclick={backupLibrary}
                    >
                      <Icon name="database" size={14} />
                      Backup library
                    </button>
                    <button
                      class="btn-ghost px-3 py-2 text-xs"
                      disabled={dataBusy}
                      onclick={restoreBackup}
                    >
                      <Icon name="refresh" size={14} />
                      Restore backup
                    </button>
                    <button
                      class="btn-ghost px-3 py-2 text-xs"
                      disabled={dataBusy}
                      onclick={exportSettings}
                    >
                      <Icon name="download" size={14} />
                      Export settings
                    </button>
                    <button
                      class="btn-ghost px-3 py-2 text-xs"
                      disabled={dataBusy}
                      onclick={importSettings}
                    >
                      <Icon name="folder" size={14} />
                      Import settings
                    </button>
                  </div>
                  <p
                    role="status"
                    class="data-status"
                    class:is-error={dataStatusTone === "error"}
                    class:is-success={dataStatusTone === "success"}
                  >
                    {dataStatus}
                    {#if restartNeeded}
                      <button class="btn-ghost px-2.5 py-1 text-[11px]" onclick={restartNow}>
                        <Icon name="refresh" size={12} />
                        Restart now
                      </button>
                    {/if}
                  </p>
                </div>
              {/if}
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
            Windows anime player. AniList discovery, user-installed sources, local SQLite library. Not an Aniyomi client.
          </p>
        </div>
        {#if onOpenOnboarding}
          <button class="btn-ghost px-3.5 py-2 text-xs" onclick={onOpenOnboarding}>
            <Icon name="sparkles" size={14} />
            Getting started
          </button>
        {/if}
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

  .anilist-avatar {
    width: 36px;
    height: 36px;
    flex-shrink: 0;
    border-radius: 50%;
    object-fit: cover;
    background: var(--tint-1);
    border: 1px solid var(--line-volt);
  }

  .data-actions {
    margin-top: 4px;
  }

  .data-status {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    min-height: 16px;
    margin-top: 10px;
    font-size: 12px;
    color: var(--text-3);
  }

  .data-status.is-success {
    color: var(--success);
  }

  .data-status.is-error {
    color: var(--danger);
  }

  .update-block {
    padding-bottom: 10px;
  }

  .update-version {
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--paper-50);
  }

  .update-status {
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
    color: var(--text-3);
  }

  .update-status.is-success {
    color: var(--success);
  }

  .update-status.is-warning {
    color: var(--warning);
  }

  .update-status.is-error {
    color: var(--danger);
  }

  .update-message {
    font-size: 12px;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }

  .update-release {
    margin-top: 10px;
    padding: 10px 11px;
    border-radius: 11px;
    background: var(--tint-1);
    border: 1px solid var(--line);
  }

  .update-meta {
    font-size: 11px;
    color: var(--text-3);
  }

  .update-notes {
    margin-top: 6px;
    font-size: 12px;
    line-height: 1.55;
    color: var(--text-2);
    white-space: pre-line;
  }

  .update-progress {
    margin-top: 10px;
  }

  .update-note {
    margin-top: 8px;
    font-size: 11px;
    line-height: 1.5;
    color: var(--text-4);
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

  .accent-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    margin-top: 16px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }

  .accent-options {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-left: auto;
  }

  .accent-option {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 7px 10px;
    border-radius: 10px;
    background: var(--control-bg);
    border: 1px solid var(--line);
    color: var(--text-3);
    font-size: 11px;
    font-weight: 600;
    transition:
      background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-out);
  }

  .accent-option:hover {
    background: var(--tint-1);
    border-color: var(--line-strong);
    color: var(--text-1);
    transform: translateY(-1px);
  }

  .accent-option.is-active {
    background: var(--accent-wash);
    border-color: var(--line-volt);
    color: var(--text-1);
    box-shadow: inset 0 0 0 1px var(--accent-wash-strong);
  }

  .accent-swatch {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--swatch);
    box-shadow:
      0 0 0 1px rgb(255 255 255 / 0.18),
      0 0 10px color-mix(in oklab, var(--swatch) 45%, transparent);
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
