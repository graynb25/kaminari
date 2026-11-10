<script lang="ts">
  import { onMount, tick } from "svelte";
  import { confirm, open } from "@tauri-apps/plugin-dialog";
  import Callout from "./Callout.svelte";
  import EmptyState from "./EmptyState.svelte";
  import Icon from "./Icon.svelte";
  import MpvView from "./MpvView.svelte";
  import PlayerView from "./PlayerView.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import {
    downloadCancel,
    downloadConsentAccepted,
    downloadFileName,
    downloadKind,
    downloadRowKey,
    downloadStart,
    onDownloadProgress,
    readDownloadsFolder,
    saveDownloadsFolder,
    setDownloadConsent,
  } from "./downloads";
  import {
    jsExtensionDetail,
    jsExtensionEpisodes,
    jsExtensionResolve,
    jsExtensionTest,
    detail as fetchAniListDetail,
    watchedMarkAll,
    watchedSet,
    mpvEmbedStatus,
    playerPlay,
    proxyRegister,
    proxyRevoke,
    proxyStatus,
    sourceLibraryAdd,
    sourceLibraryRemove,
    sourceProgressGet,
    sourceProgressMarkAll,
    sourceProgressSet,
  } from "./api";
  import { libmpvEnabled, libmpvNeedsProxy } from "./libmpv";
  import {
    pickStream,
    readQualityPreference,
    saveQualityPreference,
    type QualityPreference,
  } from "./quality";
  import QualitySelect from "./QualitySelect.svelte";
  import { autoplayEnabled } from "./autoplay";
  import { anilistProgressThrough, sourceProgressMarks } from "./sourceProgress";
  import { isHlsStream, proxyFlagEnabled, shouldProxy } from "./streamProxy";
  import type {
    DownloadProgress,
    JsEpisode,
    JsSearchItem,
    JsShowDetail,
    JsStream,
    Media,
    SourceExtensionRef,
    SourceLibraryItem,
    SourceProgress,
  } from "./types";

  let {
    extension,
    initialShow = null,
    initialEpisodeId = null,
    anilistMedia = null,
    anilistId = null,
    sourceLibrary = [],
    incognito = false,
    onLibraryChanged,
    onClose,
    notify,
  }: {
    extension: SourceExtensionRef;
    initialShow?: JsSearchItem | null;
    initialEpisodeId?: string | null;
    anilistMedia?: Media | null;
    anilistId?: number | null;
    sourceLibrary?: SourceLibraryItem[];
    incognito?: boolean;
    onLibraryChanged?: () => void;
    onClose: () => void;
    notify: (text: string, tone?: "info" | "success" | "error") => void;
  } = $props();

  let query = $state("");
  let qualityPreference = $state<QualityPreference>(readQualityPreference());
  let results = $state<JsSearchItem[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let selected = $state<JsShowDetail | null>(null);
  let episodes = $state<JsEpisode[]>([]);
  let showLoading = $state(false);
  let showError = $state<string | null>(null);
  let streams = $state<Record<string, JsStream[]>>({});
  let resolving = $state<string | null>(null);
  let markBusy = $state(false);
  let streamError = $state<string | null>(null);
  let activePlayback = $state<{
    stream: JsStream;
    episode: JsEpisode;
    nextEpisode: JsEpisode | null;
    startSec: number;
    backend: "mpv" | "web";
  } | null>(null);
  const autoplayNext = autoplayEnabled();
  let progress = $state<SourceProgress[]>([]);
  let downloadIds = $state<Record<string, string>>({});
  let downloadProgress = $state<Record<string, DownloadProgress>>({});
  let downloadPending = $state<string | null>(null);
  let searchInput: HTMLInputElement | null = $state(null);
  let dialog: HTMLDivElement | null = $state(null);
  let closeButton: HTMLButtonElement | null = $state(null);
  let previousFocus: HTMLElement | null = null;
  let autoPlayed = false;
  let playbackEpisodeId: string | null = null;
  let progressSave: Promise<void> = Promise.resolve();
  let linkedMedia = $state<Media | null>(null);
  let currentAniListId = $state<number | null>(null);
  const linkedKeys = new Set<string>();
  const anilistMarked = new Set<string>();

  const inSourceLibrary = $derived(
    selected
      ? sourceLibrary.some(
          (item) =>
            item.extensionId === extension.id && item.showId === selected?.id,
        )
      : false,
  );
  const activeAniListId = $derived(currentAniListId);
  const watchedSourceCount = $derived(progress.filter((row) => row.watched).length);
  const activeQualityOptions = $derived(
    activePlayback ? (streams[activePlayback.episode.id] ?? [activePlayback.stream]) : [],
  );

  onMount(() => {
    previousFocus = document.activeElement as HTMLElement | null;
    document.body.dataset.overlay = "source";
    requestAnimationFrame(() => {
      if (searchInput?.offsetParent) searchInput.focus();
      else closeButton?.focus();
    });
    void (async () => {
      if (anilistMedia) {
        linkedMedia = anilistMedia;
        currentAniListId = anilistMedia.id;
      }
      if (initialShow) {
        await openShow(initialShow);
      } else {
        await runSearch();
      }
    })();
    searchInput?.focus();

    return () => {
      delete document.body.dataset.overlay;
      requestAnimationFrame(() => {
        if (previousFocus?.isConnected) {
          previousFocus.focus();
          return;
        }
        const showId = initialShow?.id ?? selected?.id ?? null;
        if (!showId) return;
        const key = `${extension.id}|${showId}`;
        const target = Array.from(
          document.querySelectorAll<HTMLButtonElement>("[data-source-show]"),
        ).find((element) => element.dataset.sourceShow === key);
        target?.focus();
      });
    };
  });

  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void onDownloadProgress((entry) => {
      downloadProgress = { ...downloadProgress, [entry.id]: entry };
      if (entry.state === "done") {
        notify(entry.message ?? "Download saved.", "success");
      } else if (entry.state === "error") {
        notify(entry.message ?? "Download failed.", "error");
      }
    })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {
        // The row still shows the last known state without live events.
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  $effect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || activePlayback) return;
      event.preventDefault();
      onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function trapTab(event: KeyboardEvent) {
    if (event.key !== "Tab" || !dialog) return;
    const items = [...dialog.querySelectorAll("button, input, select")].filter(
      (el) => (el as HTMLElement).tabIndex >= 0,
    ) as HTMLElement[];
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

  async function runSearch() {
    loading = true;
    error = null;
    try {
      const result = await jsExtensionTest(extension.id, query.trim());
      results = result.items ?? [];
      if (results.length === 0) {
        error = query.trim()
          ? `No results for "${query.trim()}".`
          : "The source returned an empty shelf. Try a search term.";
      }
    } catch (e) {
      results = [];
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function openShow(item: JsSearchItem) {
    selected = {
      id: item.id,
      title: item.title,
      cover: item.cover ?? null,
      year: item.year ?? null,
    };
    applyAniListLink(item.id);
    episodes = [];
    streams = {};
    showError = null;
    streamError = null;
    showLoading = true;
    try {
      selected = await jsExtensionDetail(extension.id, item.id);
    } catch (e) {
      showError = e instanceof Error ? e.message : String(e);
    }
    if (anilistMedia) {
      await persistAniListLink();
    }
    try {
      episodes = await jsExtensionEpisodes(extension.id, item.id);
    } catch (e) {
      if (!showError) {
        showError = e instanceof Error ? e.message : String(e);
      }
    } finally {
      showLoading = false;
    }
    try {
      progress = await sourceProgressGet(extension.id, item.id);
    } catch {
      progress = [];
    }

    // A Continue card carries the episode id, so opening it should finish the
    // job: resolve that episode and resume at the saved position.
    if (initialEpisodeId && !autoPlayed && initialShow?.id === item.id) {
      autoPlayed = true;
      const episode = episodes.find((row) => row.id === initialEpisodeId);
      if (episode) {
        const list = await resolve(episode);
        if (list.length > 0) await playStream(list[0], episode);
      }
    }
  }

  function progressFor(episodeId: string): SourceProgress | undefined {
    return progress.find((row) => row.episodeId === episodeId);
  }

  function resumeFor(episodeId: string): number {
    const row = progressFor(episodeId);
    if (!row || row.watched || row.positionSec <= 0) return 0;
    if (row.durationSec > 0 && row.positionSec / row.durationSec >= 0.95) return 0;
    return row.positionSec;
  }

  function episodeAction(episode: JsEpisode): string {
    const row = progressFor(episode.id);
    if (row?.watched) return "Rewatch";
    return resumeFor(episode.id) > 0 ? "Resume" : "Play";
  }

  async function toggleSourceLibrary() {
    if (!selected) return;
    try {
      if (inSourceLibrary) {
        await sourceLibraryRemove(extension.id, selected.id);
        notify(`Removed ${selected.title} from the source library.`, "success");
      } else {
        await sourceLibraryAdd({
          extensionId: extension.id,
          showId: selected.id,
          title: selected.title,
          cover: selected.cover ?? null,
          year: selected.year ?? null,
          status: selected.status ?? null,
          anilistId: activeAniListId,
        });
        notify(`Saved ${selected.title} to the source library.`, "success");
      }
      onLibraryChanged?.();
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Source library change failed: ${message}`, "error");
    }
  }

  async function ensureSourceLibrary() {
    if (!selected || inSourceLibrary) return;
    await sourceLibraryAdd({
      extensionId: extension.id,
      showId: selected.id,
      title: selected.title,
      cover: selected.cover ?? null,
      year: selected.year ?? null,
      status: selected.status ?? null,
      anilistId: activeAniListId,
    });
  }

  async function loadAniListMedia(id: number) {
    try {
      const media = await fetchAniListDetail(id);
      if (currentAniListId === id) linkedMedia = media;
    } catch {
      // The durable link still exists; progress mirrors once AniList answers.
    }
  }

  function applyAniListLink(showId: string) {
    if (anilistMedia) {
      currentAniListId = anilistMedia.id;
      linkedMedia = anilistMedia;
      return;
    }
    const linkedRow = sourceLibrary.find(
      (item) => item.extensionId === extension.id && item.showId === showId,
    );
    const durableId =
      linkedRow?.anilistId ??
      (initialShow?.id === showId ? anilistId : null) ??
      null;
    currentAniListId = durableId;
    if (durableId === null) {
      linkedMedia = null;
      return;
    }
    if (linkedMedia?.id !== durableId) {
      linkedMedia = null;
      void loadAniListMedia(durableId);
    }
  }

  async function persistAniListLink() {
    if (!selected || !anilistMedia) return;
    const key = `${extension.id}|${selected.id}`;
    if (linkedKeys.has(key)) return;
    linkedKeys.add(key);
    try {
      await sourceLibraryAdd({
        extensionId: extension.id,
        showId: selected.id,
        title: selected.title,
        cover: selected.cover ?? null,
        year: selected.year ?? null,
        status: selected.status ?? null,
        anilistId: anilistMedia.id,
      });
      onLibraryChanged?.();
    } catch (error) {
      linkedKeys.delete(key);
      notify(`Could not link this show to AniList: ${errorText(error)}`, "error");
    }
  }

  function anilistEpisodeNumber(episode: JsEpisode): number {
    const number = Math.max(1, Math.round(episode.number));
    return linkedMedia?.episodes && linkedMedia.episodes > 0
      ? Math.min(number, linkedMedia.episodes)
      : number;
  }

  async function setSourceEpisodeWatched(episode: JsEpisode, watched: boolean) {
    if (!selected || markBusy) return;
    if (incognito) {
      notify("Incognito mode is on. Turn it off in Settings to save progress.", "info");
      return;
    }
    markBusy = true;
    try {
      if (watched) await ensureSourceLibrary();
      const duration = Math.max(0, episode.durationSec ?? 0);
      if (watched) {
        await sourceProgressSet({
          extensionId: extension.id,
          showId: selected.id,
          episodeId: episode.id,
          episodeNumber: Math.round(episode.number),
          episodeTitle: episode.title ?? null,
          positionSec: duration,
          durationSec: duration,
          watched: true,
        });
      } else {
        await sourceProgressMarkAll({
          extensionId: extension.id,
          showId: selected.id,
          episodes: sourceProgressMarks([episode]),
          watched: false,
        });
      }
      progress = await sourceProgressGet(extension.id, selected.id);
      if (watched && linkedMedia) {
        try {
          await watchedSet(linkedMedia, anilistEpisodeNumber(episode), true);
          notify(`Marked episode ${episode.number} watched and queued AniList.`, "success");
        } catch (error) {
          notify(`Marked locally, but AniList queue failed: ${errorText(error)}`, "error");
        }
      } else {
        notify(
          watched
            ? `Marked episode ${episode.number} watched.`
            : `Marked episode ${episode.number} unwatched locally.`,
          "success",
        );
      }
      onLibraryChanged?.();
    } catch (error) {
      notify(`Episode update failed: ${errorText(error)}`, "error");
    } finally {
      markBusy = false;
    }
  }

  async function setAllSourceWatched(watched: boolean) {
    if (!selected || episodes.length === 0 || markBusy) return;
    if (incognito) {
      notify("Incognito mode is on. Turn it off in Settings to save progress.", "info");
      return;
    }
    markBusy = true;
    try {
      if (watched) await ensureSourceLibrary();
      await sourceProgressMarkAll({
        extensionId: extension.id,
        showId: selected.id,
        episodes: sourceProgressMarks(episodes),
        watched,
      });
      progress = await sourceProgressGet(extension.id, selected.id);
      if (watched && linkedMedia) {
        const through = anilistProgressThrough(linkedMedia.episodes, episodes);
        if (through > 0) {
          try {
            await watchedMarkAll(linkedMedia, through, true);
            notify(
              `Marked all ${episodes.length} episodes watched and queued AniList through episode ${through}.`,
              "success",
            );
          } catch (error) {
            notify(`Marked all locally, but AniList queue failed: ${errorText(error)}`, "error");
          }
        } else {
          notify(`Marked all ${episodes.length} episodes watched.`, "success");
        }
      } else {
        notify(
          watched
            ? `Marked all ${episodes.length} episodes watched.`
            : "Cleared local source progress. AniList progress was not changed.",
          "success",
        );
      }
      onLibraryChanged?.();
    } catch (error) {
      notify(`Episode update failed: ${errorText(error)}`, "error");
    } finally {
      markBusy = false;
    }
  }

  async function saveProgress(position: number, duration: number, watched: boolean) {
    if (!selected || !activePlayback) return;
    if (incognito) return;
    const episode = activePlayback.episode;
    try {
      await sourceProgressSet({
        extensionId: extension.id,
        showId: selected.id,
        episodeId: episode.id,
        episodeNumber: episode.number,
        episodeTitle: episode.title ?? null,
        positionSec: position,
        durationSec: duration,
        watched,
      });
    } catch {
      // Progress is best effort. Playback stays up.
    }

    if (watched && linkedMedia && !anilistMarked.has(episode.id)) {
      anilistMarked.add(episode.id);
      const bounded = anilistEpisodeNumber(episode);
      try {
        await watchedSet(linkedMedia, bounded, true);
        notify(`AniList progress queued for episode ${bounded}.`, "info");
      } catch (error) {
        anilistMarked.delete(episode.id);
        notify(`Could not queue AniList progress: ${errorText(error)}`, "error");
      }
    }
  }

  async function closePlayer() {
    const proxyIds = [
      activePlayback?.stream.proxyId ?? null,
      ...(activePlayback?.stream.subtitleProxyIds ?? []),
    ].filter((id): id is string => Boolean(id));
    const episodeId = activePlayback?.episode.id ?? playbackEpisodeId;
    activePlayback = null;
    await tick();
    focusEpisodePlay(episodeId);
    playbackEpisodeId = null;
    await progressSave;
    for (const proxyId of proxyIds) {
      try {
        await proxyRevoke(proxyId);
      } catch {
        // Revoke is best effort. The entry also expires on its own.
      }
    }
    if (!selected) return;
    try {
      progress = await sourceProgressGet(extension.id, selected.id);
    } catch {
      // Keep the old progress on a read failure.
    }
    onLibraryChanged?.();
  }

  function focusEpisodePlay(episodeId: string | null) {
    if (!episodeId) return;
    const buttons = document.querySelectorAll<HTMLButtonElement>("[data-episode-play]");
    for (const button of buttons) {
      if (button.dataset.episodePlay === episodeId) {
        button.focus();
        return;
      }
    }
  }

  async function resolve(episode: JsEpisode): Promise<JsStream[]> {
    resolving = episode.id;
    streamError = null;
    try {
      const list = await jsExtensionResolve(extension.id, episode.id);
      streams = { ...streams, [episode.id]: list };
      if (!list || list.length === 0) {
        streamError = "The source returned no streams for this episode.";
      }
      return list ?? [];
    } catch (e) {
      streamError = e instanceof Error ? e.message : String(e);
      return [];
    } finally {
      resolving = null;
    }
  }

  async function copyStream(url: string) {
    try {
      await navigator.clipboard.writeText(url);
      notify("Stream URL copied.", "success");
    } catch {
      notify("Could not copy the URL.", "error");
    }
  }

  function errorText(error: unknown): string {
    if (typeof error === "string") return error;
    if (error instanceof Error) return error.message;
    return "Unknown error.";
  }

  async function ensureDownloadConsent(): Promise<boolean> {
    if (downloadConsentAccepted()) return true;
    try {
      const accepted = await confirm(
        "Only download content you have the right to keep.",
        { title: "Download stream", kind: "warning" },
      );
      if (!accepted) return false;
      setDownloadConsent(true);
      return true;
    } catch (error) {
      notify(`Could not open the download prompt: ${errorText(error)}`, "error");
      return false;
    }
  }

  async function chooseDownloadFolder(): Promise<string | null> {
    try {
      const picked = await open({
        title: "Choose downloads folder",
        directory: true,
        multiple: false,
      });
      if (typeof picked !== "string" || !picked.trim()) return null;
      saveDownloadsFolder(picked);
      return picked;
    } catch (error) {
      notify(`Could not open the folder picker: ${errorText(error)}`, "error");
      return null;
    }
  }

  async function downloadsFolder(): Promise<string | null> {
    const saved = readDownloadsFolder();
    if (saved) return saved;
    return chooseDownloadFolder();
  }

  function downloadFor(episode: JsEpisode, stream: JsStream): DownloadProgress | null {
    const id = downloadIds[downloadRowKey(episode.id, stream)];
    return id ? (downloadProgress[id] ?? null) : null;
  }

  function formatDownloadBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function downloadLabel(entry: DownloadProgress): string {
    if (entry.state === "done") {
      return entry.path ? `Saved to ${entry.path}` : "Saved.";
    }
    if (entry.state === "error") {
      return entry.message ?? "Download failed.";
    }
    if (entry.state === "cancelled") {
      return entry.message ?? "Download cancelled.";
    }
    const received = formatDownloadBytes(entry.received);
    if (entry.total && entry.total > 0) {
      return `${received} of ${formatDownloadBytes(entry.total)}`;
    }
    return received;
  }

  async function startDownload(stream: JsStream, episode: JsEpisode) {
    const kind = downloadKind(stream);
    if (!kind) {
      notify("Only MP4, OGV and simple HLS streams can be downloaded.", "info");
      return;
    }
    const key = downloadRowKey(episode.id, stream);
    if (downloadFor(episode, stream)?.state === "running" || downloadPending === key) {
      return;
    }
    downloadPending = key;
    try {
      if (!(await ensureDownloadConsent())) return;
      const folder = await downloadsFolder();
      if (!folder) return;
      const started = await downloadStart({
        url: stream.url,
        proxiedUrl: stream.proxiedUrl ?? null,
        headers: stream.headers ?? null,
        kind,
        folder,
        fileName: downloadFileName(
          selected?.title ?? "Kaminari",
          episode.number,
          stream,
        ),
      });
      downloadIds = { ...downloadIds, [key]: started.id };
      if (!downloadProgress[started.id]) {
        downloadProgress = {
          ...downloadProgress,
          [started.id]: {
            id: started.id,
            received: 0,
            total: null,
            state: "running",
            path: null,
            message: null,
          },
        };
      }
    } catch (error) {
      notify(`Download could not start: ${errorText(error)}`, "error");
    } finally {
      downloadPending = null;
    }
  }

  async function cancelDownload(id: string) {
    try {
      await downloadCancel(id);
    } catch (error) {
      notify(`Download could not be cancelled: ${errorText(error)}`, "error");
    }
  }

  async function playStream(stream: JsStream, episode: JsEpisode, startSecOverride?: number) {
    let prepared = stream;
    if (shouldProxy(stream) && proxyFlagEnabled()) {
      try {
        const status = await proxyStatus();
        if (status.enabled) {
          const registered = await proxyRegister(extension.id, {
            url: stream.url,
            kind: isHlsStream(stream) ? "hls" : "mp4",
            headers: stream.headers ?? null,
          });
          const subtitles: NonNullable<JsStream["subtitles"]> = [];
          const subtitleProxyIds: string[] = [];
          for (const subtitle of stream.subtitles ?? []) {
            if (!/^https?:\/\//i.test(subtitle.url)) {
              subtitles.push(subtitle);
              continue;
            }
            try {
              const proxied = await proxyRegister(extension.id, {
                url: subtitle.url,
                kind: "file",
                headers: stream.headers ?? null,
              });
              subtitles.push({ ...subtitle, url: proxied.proxyUrl });
              subtitleProxyIds.push(proxied.opaqueId);
            } catch {
              subtitles.push(subtitle);
            }
          }
          prepared = {
            ...stream,
            proxiedUrl: registered.proxyUrl,
            proxyId: registered.opaqueId,
            subtitles: subtitles.length > 0 ? subtitles : stream.subtitles,
            subtitleProxyIds,
          };
        }
      } catch {
        notify("The protected stream could not be proxied. Trying it directly.", "info");
      }
    }
    let backend: "mpv" | "web" = "web";
    const libmpvOn = libmpvEnabled();
    if (libmpvOn && libmpvNeedsProxy(prepared)) {
      notify(
        shouldProxy(prepared)
          ? "libmpv needs the loopback proxy for this stream's headers. Using the WebView player."
          : "This stream type cannot use the loopback proxy. Open it in mpv. Using the WebView player.",
        "info",
      );
    } else if (libmpvOn) {
      try {
        const status = await mpvEmbedStatus();
        if (status.available) {
          backend = "mpv";
        } else {
          notify(
            `libmpv is unavailable: ${status.lastError ?? "DLLs are missing"}. Using the WebView player.`,
            "info",
          );
        }
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        notify(`libmpv is unavailable: ${message}. Using the WebView player.`, "info");
      }
    }
    activePlayback = {
      stream: prepared,
      episode,
      nextEpisode: nextEpisodeFor(episode),
      startSec: startSecOverride ?? resumeFor(episode.id),
      backend,
    };
    playbackEpisodeId = episode.id;
  }

  async function switchQuality(stream: JsStream, position: number) {
    if (!activePlayback || stream.url === activePlayback.stream.url) return;
    const episode = activePlayback.episode;
    await closePlayer();
    await playStream(stream, episode, position);
  }

  function nextEpisodeFor(episode: JsEpisode): JsEpisode | null {
    const index = episodes.findIndex((row) => row.id === episode.id);
    if (index < 0) return null;
    return episodes[index + 1] ?? null;
  }

  async function playNext() {
    const next = activePlayback?.nextEpisode;
    if (!next) return;
    await closePlayer();
    await playEpisode(next);
  }

  function fallbackToWebView(message: string) {
    if (!activePlayback) return;
    notify(`libmpv could not play this stream: ${message}. Using the WebView player.`, "error");
    activePlayback = { ...activePlayback, backend: "web" };
  }

  async function playEpisode(episode: JsEpisode) {
    const cached = streams[episode.id];
    const list = cached && cached.length > 0 ? cached : await resolve(episode);
    const picked = pickStream(list, qualityPreference);
    if (picked) await playStream(picked, episode);
  }

  function chooseQuality(value: QualityPreference) {
    qualityPreference = value;
    saveQualityPreference(qualityPreference);
  }

  function toggleStreams(episode: JsEpisode) {
    if (streams[episode.id]) {
      const next = { ...streams };
      delete next[episode.id];
      streams = next;
      return;
    }
    void resolve(episode);
  }

  async function openExternal(stream: JsStream, episode: JsEpisode) {
    try {
      const info = await playerPlay({
        url: stream.url,
        title: `${selected?.title ?? "Kaminari"} · Episode ${episode.number}`,
        headers: stream.headers ?? undefined,
        subtitles: stream.subtitles ?? undefined,
      });
      notify(`Opening mpv from ${info.mpvPath}.`, "success");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notify(`Could not open mpv: ${message}`, "error");
    }
  }

  function backToResults() {
    selected = null;
    episodes = [];
    streams = {};
    showError = null;
    streamError = null;
  }

  function duration(seconds?: number | null): string {
    if (!seconds || seconds <= 0) return "";
    const minutes = Math.floor(seconds / 60);
    const rest = seconds % 60;
    return `${minutes}:${String(rest).padStart(2, "0")}`;
  }

  function formatTime(value: number): string {
    if (!Number.isFinite(value) || value < 0) return "0:00";
    const hours = Math.floor(value / 3600);
    const minutes = Math.floor((value % 3600) / 60);
    const seconds = Math.floor(value % 60);
    return hours > 0
      ? `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`
      : `${minutes}:${String(seconds).padStart(2, "0")}`;
  }

  function clean(text?: string | null): string {
    if (!text) return "";
    return text
      .replace(/<br\s*\/?>/gi, " ")
      .replace(/<[^>]+>/g, " ")
      .replace(/&quot;/g, '"')
      .replace(/&#0?39;/g, "'")
      .replace(/&amp;/g, "&")
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/\s+/g, " ")
      .trim();
  }

  function hostOf(url: string): string {
    try {
      return new URL(url).host;
    } catch {
      return url.slice(0, 42);
    }
  }
</script>

<div
  bind:this={dialog}
  class="fixed inset-0 z-40 flex flex-col"
  class:is-playing={activePlayback !== null}
  style="background: radial-gradient(900px 500px at 20% -10%, var(--accent-wash), transparent 60%), var(--void-950);"
  role="dialog"
  aria-modal="true"
  aria-label="{extension.name} source browser"
  tabindex={-1}
  onkeydown={trapTab}
>
  <header class="source-browser-head h-14 shrink-0 flex items-center gap-3 px-4 glass-strong">
    <button bind:this={closeButton} class="btn-ghost px-2.5 py-2 text-xs" onclick={onClose} aria-label="Close source browser">
      <Icon name="close" size={15} />
    </button>
    <div class="source-tile" style="width: 34px; height: 34px; font-size: 10px;" aria-hidden="true">JS</div>
    <div class="min-w-0">
      <p class="text-sm font-semibold truncate">{extension.name}</p>
      <p class="text-[10px] truncate" style="color: var(--paper-450); font-family: var(--font-mono);">
        {extension.id}
      </p>
    </div>
    <form
      class="ml-auto flex items-center gap-2"
      onsubmit={(event) => {
        event.preventDefault();
        void runSearch();
      }}
    >
      <label class="relative hidden sm:block">
        <span class="sr-only">Search this source</span>
        <span class="absolute left-3 top-1/2 -translate-y-1/2" style="color: var(--paper-450);">
          <Icon name="search" size={13} />
        </span>
        <input
          bind:this={searchInput}
          bind:value={query}
          placeholder="Search this source"
          class="h-8 w-[240px] rounded-lg pl-8 pr-3 text-sm outline-none"
          style="background: var(--input-bg); border: 1px solid var(--line); color: var(--text-1);"
          aria-label="Search this source"
        />
      </label>
      <button type="submit" class="btn-primary px-3 py-1.5 text-[11px]" disabled={loading}>
        {loading ? "Searching" : "Search"}
      </button>
    </form>
  </header>

  <main class="flex-1 overflow-y-auto">
    {#if selected}
      <div class="max-w-[1200px] mx-auto p-6 md:p-8">
        <div class="flex flex-wrap items-center gap-2 mb-4">
          <button class="btn-ghost px-3 py-1.5 text-xs" onclick={backToResults}>
            <Icon name="arrowLeft" size={14} />
            Back to results
          </button>
          <button
            class={inSourceLibrary ? "btn-ghost" : "btn-primary"}
            class:px-3={true}
            class:py-1.5={true}
            class:text-xs={true}
            onclick={() => void toggleSourceLibrary()}
          >
            <Icon name={inSourceLibrary ? "check" : "plus"} size={14} />
            {inSourceLibrary ? "In Library" : "Library"}
          </button>
          <div class="ml-auto flex items-center gap-2 text-[11px]" style="color: var(--paper-450);">
            <span>Quality</span>
            <QualitySelect
              value={qualityPreference}
              onChange={chooseQuality}
              label="Preferred playback quality"
              compact
            />
          </div>
        </div>

        <div class="relative overflow-hidden rounded-3xl" style="border: 1px solid var(--line-strong); box-shadow: var(--shadow-pop);">
          {#if selected.banner || selected.cover}
            <img src={selected.banner || selected.cover} alt="" class="absolute inset-0 w-full h-full object-cover blur-2xl scale-110 opacity-40" />
          {/if}
          <div class="source-hero-scrim absolute inset-0" aria-hidden="true"></div>
          <div class="relative flex flex-col sm:flex-row gap-6 p-6 md:p-8">
            <div
              class="w-[150px] sm:w-[190px] shrink-0 aspect-[2/3] rounded-2xl overflow-hidden"
              style="background: var(--void-800); border: 1px solid var(--line-strong); box-shadow: 0 20px 50px rgb(0 0 0 / 0.55);"
            >
              {#if selected.cover}
                <img src={selected.cover} alt="" class="w-full h-full object-cover" />
              {:else}
                <div class="w-full h-full grid place-items-center">
                  <span class="text-4xl font-bold" style="color: var(--paper-450); font-family: var(--font-display);">{selected.title.slice(0, 1)}</span>
                </div>
              {/if}
            </div>
            <div class="min-w-0 flex-1 self-end">
              <p class="eyebrow">Source result</p>
              <h1 class="mt-1 leading-[1.04] line-clamp-2" style="font-family: var(--font-display); font-size: clamp(28px, 3.6vw, 46px); font-weight: 750; letter-spacing: -0.03em;">
                {selected.title}
              </h1>
              <div class="flex flex-wrap items-center gap-2 mt-2.5">
                {#if selected.year}<span class="chip text-[11px] px-2.5 py-1">{selected.year}</span>{/if}
                {#if selected.status}<span class="chip text-[11px] px-2.5 py-1">{selected.status}</span>{/if}
                <span class="chip chip-volt text-[11px] px-2.5 py-1">
                  <Icon name="package" size={11} />
                  {extension.name}
                </span>
                {#if activeAniListId}
                  <span class="chip text-[11px] px-2.5 py-1" title="Finished episodes are queued to AniList">
                    <Icon name="check" size={11} />
                    AniList
                  </span>
                {/if}
              </div>
              {#if selected.genres?.length}
                <div class="flex flex-wrap gap-1.5 mt-2.5">
                  {#each selected.genres as genre}
                    <span class="chip text-[10px] px-2 py-0.5">{genre}</span>
                  {/each}
                </div>
              {/if}
              <p class="text-sm mt-3 leading-relaxed max-w-2xl line-clamp-3" style="color: var(--paper-300);">
                {clean(selected.synopsis) || "This source did not return a synopsis."}
              </p>
            </div>
          </div>
        </div>

        {#if showError}
          <Callout tone="danger" class="mt-4">{showError}</Callout>
        {/if}

        <div class="flex flex-wrap items-center gap-x-3 gap-y-2 mt-8 mb-3">
          <div class="flex items-baseline gap-3">
            <h2 class="shelf-title">Episodes</h2>
            <span class="shelf-count">{episodes.length}</span>
          </div>
          <div class="ml-auto flex flex-wrap items-center gap-1.5">
            {#if incognito}
              <span
                class="chip text-[10px] px-2 py-0.5"
                title="Playback and watched changes are not saved while Incognito is on"
              >
                <Icon name="eyeOff" size={10} />
                Incognito
              </span>
            {/if}
            <span class="text-[10px] mr-1" style="color: var(--paper-450); font-family: var(--font-mono);">
              {watchedSourceCount} watched
            </span>
            <button
              class="btn-ghost px-2.5 py-1.5 text-[11px]"
              disabled={incognito || markBusy || episodes.length === 0 || watchedSourceCount === episodes.length}
              onclick={() => void setAllSourceWatched(true)}
              title={incognito
                ? "Incognito mode is on. Turn it off in Settings to save progress."
                : "Mark every source episode watched without playing it. Queues AniList progress when linked."}
            >
              <Icon name="check" size={12} />
              Mark all watched
            </button>
            <button
              class="btn-ghost px-2.5 py-1.5 text-[11px]"
              disabled={incognito || markBusy || watchedSourceCount === 0}
              onclick={() => void setAllSourceWatched(false)}
              title={incognito
                ? "Incognito mode is on. Turn it off in Settings to save progress."
                : "Clear source progress locally. AniList progress is unchanged."}
            >
              Clear
            </button>
          </div>
        </div>

        {#if showLoading}
          <div class="space-y-2">
            {#each Array(4) as _}
              <div class="surface p-3 flex items-center gap-3">
                <div class="shimmer w-24 h-14 rounded-lg shrink-0"></div>
                <div class="flex-1 space-y-2">
                  <div class="shimmer h-3 w-1/2 rounded"></div>
                  <div class="shimmer h-2 w-1/3 rounded"></div>
                </div>
                <div class="shimmer h-8 w-20 rounded-lg"></div>
              </div>
            {/each}
          </div>
        {:else if episodes.length === 0}
          <div class="surface px-4 py-5 text-sm flex items-center gap-3" style="color: var(--paper-300);">
            <span style="color: var(--paper-450);"><Icon name="info" size={16} /></span>
            This source returned no episodes for this title.
          </div>
        {:else}
          <div class="space-y-2">
            {#each episodes as episode (episode.id)}
              {@const saved = progressFor(episode.id)}
              <div class="surface overflow-hidden">
                <div class="flex items-center gap-3 p-3">
                  <div class="episode-thumb shrink-0">
                    {#if episode.thumbnail}
                      <img src={episode.thumbnail} alt="" loading="lazy" decoding="async" />
                    {:else}
                      <div class="episode-thumb-empty" aria-hidden="true"></div>
                    {/if}
                    <span class="episode-thumb-scrim" aria-hidden="true"></span>
                    <span class="episode-thumb-number">EP {episode.number}</span>
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2">
                      <p class="text-sm font-semibold truncate">
                        {episode.title || `Episode ${episode.number}`}
                      </p>
                      {#if saved?.watched}
                        <span class="chip chip-live text-[9px] px-1.5 py-0.5">Watched</span>
                      {/if}
                    </div>
                    <p class="text-[11px] mt-0.5" style="color: var(--paper-450); font-family: var(--font-mono);">
                      EP {episode.number}{duration(episode.durationSec) ? ` · ${duration(episode.durationSec)}` : ""}
                    </p>
                    {#if saved}
                      <div class="h-0.5 rounded-full mt-1.5 overflow-hidden" style="background: var(--void-700);" aria-hidden="true">
                        <div
                          class="h-full"
                          style="width: {saved.durationSec > 0
                            ? Math.min(100, (saved.positionSec / saved.durationSec) * 100)
                            : 0}%; background: var(--volt-400);"
                        ></div>
                      </div>
                      <p class="text-[10px] mt-0.5" style="color: var(--paper-450);">
                        {saved.watched
                          ? "Watched"
                          : saved.positionSec > 0
                            ? `Resume ${formatTime(saved.positionSec)}`
                            : ""}
                      </p>
                    {/if}
                  </div>
                  <div class="flex items-center gap-1.5 shrink-0">
                    <button
                      class="btn-ghost w-8 h-8 grid place-items-center shrink-0"
                      disabled={incognito || markBusy || resolving === episode.id}
                      onclick={() => void setSourceEpisodeWatched(episode, !(saved?.watched ?? false))}
                      aria-pressed={saved?.watched ?? false}
                      aria-label={saved?.watched
                        ? `Mark episode ${episode.number} unwatched`
                        : `Mark episode ${episode.number} watched`}
                      title={incognito
                        ? "Incognito mode is on. Turn it off in Settings to save progress."
                        : saved?.watched
                          ? `Mark episode ${episode.number} unwatched (local source progress)`
                          : `Mark episode ${episode.number} watched without playing`}
                      style={saved?.watched ? "color: var(--volt-300);" : "color: var(--paper-450);"}
                    >
                      <Icon name={saved?.watched ? "checkCircle" : "check"} size={14} />
                    </button>
                    <button
                      class="btn-primary px-3 py-1.5 text-[11px] shrink-0"
                      data-episode-play={episode.id}
                      disabled={resolving === episode.id}
                      onclick={() => void playEpisode(episode)}
                      aria-label="{resolving === episode.id ? 'Resolving' : episodeAction(episode)} episode {episode.number}"
                    >
                      <Icon name="playCircle" size={13} />
                      {resolving === episode.id ? "Resolving" : episodeAction(episode)}
                    </button>
                    <button
                      class="btn-ghost px-2.5 py-1.5 text-[11px] shrink-0"
                      disabled={resolving === episode.id}
                      onclick={() => toggleStreams(episode)}
                      aria-label="{streams[episode.id] ? 'Hide' : 'Show'} streams for episode {episode.number}"
                    >
                      <Icon name="list" size={12} />
                      {streams[episode.id] ? "Hide" : "Streams"}
                    </button>
                  </div>
                </div>
                {#if resolving === episode.id}
                  <div class="px-3 pb-3">
                    <ProgressBar label="Resolving episode" indeterminate size="xs" />
                  </div>
                {/if}
                {#if streams[episode.id]}
                  <div class="px-3 pb-3 space-y-1.5" style="border-top: 1px solid var(--line); padding-top: 10px;">
                    {#each streams[episode.id] as stream}
                      {@const download = downloadFor(episode, stream)}
                      {@const kind = downloadKind(stream)}
                      <div class="rounded-lg" style="background: var(--well-bg); border: 1px solid var(--line);">
                        <div class="flex flex-wrap items-center gap-2 px-2.5 py-2">
                          <span class="chip chip-volt text-[9px] px-1.5 py-0.5 shrink-0">
                            {stream.type.toUpperCase()}
                          </span>
                          {#if stream.quality}
                            <span class="text-[10px] font-semibold shrink-0" style="color: var(--paper-300);">{stream.quality}</span>
                          {/if}
                          <span class="text-[10px] truncate flex-1" style="color: var(--paper-450); font-family: var(--font-mono);" title={stream.url}>
                            {hostOf(stream.url)}
                          </span>
                          <button class="btn-primary px-2.5 py-1 text-[10px] shrink-0" onclick={() => void playStream(stream, episode)}>
                            <Icon name="play" size={10} />
                            {episodeAction(episode)}
                          </button>
                          {#if kind}
                            <button
                              class="btn-ghost px-2.5 py-1 text-[10px] shrink-0"
                              disabled={download?.state === "running" || downloadPending !== null}
                              onclick={() => void startDownload(stream, episode)}
                              aria-label="Download {kind.toUpperCase()} stream"
                            >
                              <Icon name="download" size={10} />
                              Download
                            </button>
                          {/if}
                          <button class="btn-ghost px-2.5 py-1 text-[10px] shrink-0" onclick={() => void copyStream(stream.url)}>
                            <Icon name="copy" size={10} />
                            Copy
                          </button>
                          <button class="btn-ghost px-2.5 py-1 text-[10px] shrink-0" onclick={() => void openExternal(stream, episode)} aria-label="Open in mpv">
                            <Icon name="external" size={10} />
                          </button>
                        </div>
                        {#if download}
                          <div class="px-2.5 pb-2">
                            <ProgressBar
                              label="Download progress"
                              value={download.total && download.total > 0 ? download.received : null}
                              max={download.total && download.total > 0 ? download.total : 100}
                              indeterminate={download.state === "running" && !download.total}
                              tone={download.state === "error" ? "danger" : download.state === "done" ? "success" : "accent"}
                              size="xs"
                            />
                            <div class="flex flex-wrap items-center gap-2 mt-1.5">
                              <span
                                class="text-[10px] min-w-0 break-all"
                                style="color: {download.state === 'error' ? 'var(--danger)' : 'var(--paper-450)'};"
                              >
                                {downloadLabel(download)}
                              </span>
                              {#if download.state === "running"}
                                <button class="btn-ghost px-2 py-0.5 text-[10px] shrink-0" onclick={() => void cancelDownload(download.id)}>
                                  <Icon name="close" size={9} />
                                  Cancel
                                </button>
                              {:else if download.state === "done" && kind === "hls"}
                                <span class="text-[10px]" style="color: var(--paper-450);">Open this HLS file in mpv.</span>
                              {/if}
                            </div>
                          </div>
                        {/if}
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
          {#if streamError}
            <Callout tone="danger" class="mt-3">{streamError}</Callout>
          {/if}
          <p class="text-xs mt-3" style="color: var(--paper-450);">
            Play uses your preferred quality when the source offers several, and Streams lists
            every quality with Download, Copy and Open in mpv. Downloads keep the folder you pick
            in Settings.
          </p>
          <p class="text-xs mt-1" style="color: var(--paper-450);">
            The check button marks an episode watched without playing it. Mark all watched does the
            same for every episode and queues AniList progress when this show is linked.
          </p>
        {/if}
      </div>
    {:else}
      <div class="max-w-[1440px] mx-auto p-6 md:p-8">
        <div class="flex items-baseline gap-3 mb-4">
          <div>
            <p class="eyebrow">Source browser</p>
            <h2 class="shelf-title mt-1">Search results</h2>
          </div>
          <span class="shelf-count">{results.length}</span>
          <span class="ml-auto text-[10px]" style="color: var(--paper-450); font-family: var(--font-mono);">
            {extension.name}
          </span>
        </div>

        {#if loading}
          <ProgressBar label="Searching source" indeterminate class="mb-4" />
          <div class="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-6 gap-4">
            {#each Array(12) as _}
              <div class="space-y-2">
                <div class="aspect-[2/3] rounded-xl shimmer"></div>
                <div class="h-3 w-3/4 rounded shimmer"></div>
              </div>
            {/each}
          </div>
        {:else if error}
          <EmptyState
            compact
            icon="search"
            title={error.startsWith("No results") ? "No matches in this source" : "Source search failed"}
            body={error}
            actionLabel="Retry"
            actionIcon="refresh"
            onAction={() => void runSearch()}
          />
        {:else if results.length === 0}
          <EmptyState
            compact
            icon="search"
            title="Search this source"
            body="Type a title and run a search. Results come straight from the extension host."
            actionLabel="Search"
            onAction={() => void runSearch()}
          />
        {:else}
          <div class="stagger-grid grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-6 gap-4">
            {#each results as item (item.id)}
              <div>
                <button
                  class="poster-card group block w-full text-left"
                  onclick={() => void openShow(item)}
                  aria-label="Open {item.title}"
                >
                  <div class="poster-art aspect-[2/3]">
                    {#if item.cover}
                      <img src={item.cover} alt="" loading="lazy" decoding="async" class="w-full h-full object-cover" />
                    {:else}
                      <div class="w-full h-full grid place-items-center" style="background: radial-gradient(circle at 30% 20%, var(--accent-wash-strong), transparent 60%), var(--void-800);">
                        <span class="text-3xl font-bold" style="color: var(--paper-450); font-family: var(--font-display);">{item.title.slice(0, 1)}</span>
                      </div>
                    {/if}
                    <span class="poster-scrim" aria-hidden="true"></span>
                    <span class="poster-badge">Source</span>
                    {#if item.score}
                      <span class="poster-score">
                        <Icon name="star" size={9} />
                        {(item.score / 10).toFixed(1)}
                      </span>
                    {/if}
                  </div>
                  <div class="poster-body">
                    <p class="poster-title">{item.title}</p>
                    <p class="poster-meta">{item.year ?? "Source result"}{item.episodes ? ` · ${item.episodes} eps` : ""}</p>
                  </div>
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </main>
</div>

{#if activePlayback}
  {#key activePlayback.episode.id}
  {#if activePlayback.backend === "mpv"}
    <MpvView
      stream={activePlayback.stream}
      title={selected?.title ?? "Source playback"}
      subtitle="Episode {activePlayback.episode.number}{activePlayback.episode.title
        ? ` · ${activePlayback.episode.title}`
        : ''}"
      startSec={activePlayback.startSec}
      onProgress={(position, duration, watched) => {
        progressSave = saveProgress(position, duration, watched);
      }}
      onClose={() => void closePlayer()}
      onOpenExternal={() =>
        void openExternal(activePlayback!.stream, activePlayback!.episode)}
      onFallback={fallbackToWebView}
      qualityOptions={activeQualityOptions}
      onQualityChange={(stream, position) => void switchQuality(stream, position)}
      {incognito}
      {notify}
    />
  {:else}
    <PlayerView
      stream={activePlayback.stream}
      title={selected?.title ?? "Source playback"}
      subtitle="Episode {activePlayback.episode.number}{activePlayback.episode.title
        ? ` · ${activePlayback.episode.title}`
        : ''}"
      startSec={activePlayback.startSec}
      onProgress={(position, duration, watched) => {
        progressSave = saveProgress(position, duration, watched);
      }}
      onClose={() => void closePlayer()}
      onOpenExternal={() =>
        void openExternal(activePlayback!.stream, activePlayback!.episode)}
      nextLabel={activePlayback.nextEpisode
        ? `Episode ${activePlayback.nextEpisode.number}${activePlayback.nextEpisode.title ? `: ${activePlayback.nextEpisode.title}` : ""}`
        : null}
      autoplay={autoplayNext}
      onNext={() => void playNext()}
      qualityOptions={activeQualityOptions}
      onQualityChange={(stream, position) => void switchQuality(stream, position)}
      {incognito}
      {notify}
    />
  {/if}
  {/key}
{/if}

<style>
  .is-playing {
    visibility: hidden;
  }

  .source-browser-head {
    border: 0;
    border-bottom: 1px solid var(--line);
    box-shadow: none;
  }

  .episode-thumb {
    position: relative;
    width: 96px;
    height: 56px;
    overflow: hidden;
    border-radius: 10px;
    background: var(--void-700);
    border: 1px solid var(--line);
  }

  .episode-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .episode-thumb-empty {
    width: 100%;
    height: 100%;
    background:
      radial-gradient(circle at 30% 20%, var(--accent-wash-strong), transparent 60%),
      var(--void-800);
  }

  .episode-thumb-scrim {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(0deg, rgb(8 8 15 / 0.82), transparent 62%),
      linear-gradient(180deg, rgb(8 8 15 / 0.28), transparent 34%);
    pointer-events: none;
  }

  .episode-thumb-number {
    position: absolute;
    left: 5px;
    bottom: 5px;
    display: inline-flex;
    align-items: center;
    height: 17px;
    padding: 0 5px;
    border-radius: 5px;
    background: rgb(8 8 15 / 0.72);
    border: 1px solid rgb(255 255 255 / 0.18);
    color: #f4f3ff;
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.04em;
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
  }
</style>
