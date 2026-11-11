<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Icon from "./Icon.svelte";
  import {
    mpvEmbedCycleSub,
    mpvEmbedDestroy,
    mpvEmbedGet,
    mpvEmbedInit,
    mpvEmbedLoad,
    mpvEmbedSeek,
    mpvEmbedSet,
    mpvEmbedStatus,
  } from "./api";
  import { playbackUrl } from "./streamProxy";
  import PlayerQualityMenu from "./PlayerQualityMenu.svelte";
  import {
    isolateBackground,
    isTopLayer,
    popLayer,
    pushLayer,
    trapTabKey,
  } from "./a11y";
  import type { JsStream } from "./types";

  let {
    stream,
    title,
    subtitle,
    startSec = 0,
    onClose,
    onOpenExternal,
    notify,
    onProgress,
    onFallback,
    qualityOptions = [],
    onQualityChange,
    incognito = false,
  }: {
    stream: JsStream;
    title: string;
    subtitle?: string;
    startSec?: number;
    onClose: () => void;
    onOpenExternal: () => void;
    notify: (text: string, tone?: "info" | "success" | "error") => void;
    onProgress?: (position: number, duration: number, watched: boolean) => void;
    onFallback?: (message: string) => void;
    qualityOptions?: JsStream[];
    onQualityChange?: (stream: JsStream, position: number) => void;
    incognito?: boolean;
  } = $props();

  function chooseQuality(option: JsStream) {
    qualityMenuOpen = false;
    if (option.url === stream.url) return;
    onQualityChange?.(option, currentTime);
  }

  type MpvEvent = {
    event?: string;
    name?: string;
    data?: unknown;
    reason?: string;
  };

  let shell: HTMLDivElement | null = $state(null);
  let playing = $state(false);
  let buffering = $state(true);
  let currentTime = $state(0);
  let duration = $state(0);
  let volume = $state(0.9);
  let muted = $state(false);
  let speed = $state(1);
  let error = $state<string | null>(null);
  let controlsVisible = $state(true);
  let fullscreen = $state(false);
  let ended = $state(false);
  let diagnostics = $state(false);
  let qualityMenuOpen = $state(false);
  let firstFrameMs = $state<number | null>(null);
  let hwdec = $state("unknown");
  let videoCodec = $state("unknown");
  let videoFormat = $state("unknown");
  let subtitleState = $state("auto");
  let dllStatus = $state("checking");
  let windowLabel = "";
  let loadStartedAt = 0;
  let pendingStart = 0;
  let hideTimer: number | null = null;
  let clickTimer: number | null = null;
  let frameTimer: number | null = null;
  let lastReport = 0;
  let previousFocus: HTMLElement | null = null;
  let restoreBackground: (() => void) | null = null;
  let retryButton: HTMLButtonElement | null = $state(null);
  let replayButton: HTMLButtonElement | null = $state(null);
  const layer = {};
  let unlisten: UnlistenFn | null = null;
  let disposed = false;

  const subtitles = $derived(stream.subtitles ?? []);
  const hasHeaders = $derived(Object.keys(stream.headers ?? {}).length > 0);
  const playbackSource = $derived(playbackUrl(stream));
  const proxied = $derived(Boolean(stream.proxiedUrl));
  const streamHost = $derived.by(() => {
    try {
      return new URL(stream.url).host;
    } catch {
      return "local file";
    }
  });

  onMount(() => {
    const element = shell;
    previousFocus = document.activeElement as HTMLElement | null;
    document.body.dataset.playerOpen = "true";
    pushLayer(layer);
    element?.focus({ preventScroll: true });
    if (element) restoreBackground = isolateBackground(element);

    const onFocusIn = (event: FocusEvent) => {
      if (!isTopLayer(layer) || !element?.isConnected) return;
      const target = event.target;
      if (target instanceof Node && element.contains(target)) return;
      element.focus({ preventScroll: true });
    };
    document.addEventListener("focusin", onFocusIn, true);

    const onKey = (event: KeyboardEvent) => {
      if (!isTopLayer(layer)) return;
      if (event.key === "Escape") {
        event.preventDefault();
        if (qualityMenuOpen) {
          qualityMenuOpen = false;
          return;
        }
        closePlayer();
        return;
      }
      const target = event.target as HTMLElement | null;
      const tag = target?.tagName;
      if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;
      if (tag === "BUTTON" && (event.key === " " || event.key === "Enter")) return;

      const key = event.key.toLowerCase();
      if (event.key === " " || key === "k") {
        event.preventDefault();
        togglePlay();
      } else if (key === "j") {
        event.preventDefault();
        void skip(-10);
      } else if (key === "l") {
        event.preventDefault();
        void skip(10);
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        void skip(-(event.shiftKey ? 60 : 10));
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        void skip(event.shiftKey ? 60 : 10);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        void changeVolume(Math.min(1, volume + 0.05));
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        void changeVolume(Math.max(0, volume - 0.05));
      } else if (key === "m") {
        event.preventDefault();
        void toggleMute();
      } else if (key === "f") {
        event.preventDefault();
        void toggleFullscreen();
      } else if (key === "v") {
        event.preventDefault();
        void cycleSubtitles();
      } else if (event.key === ",") {
        event.preventDefault();
        void stepSpeed(-1);
      } else if (event.key === ".") {
        event.preventDefault();
        void stepSpeed(1);
      } else if (event.key === "0") {
        event.preventDefault();
        void resetSpeed();
      } else if (key === "d") {
        event.preventDefault();
        diagnostics = !diagnostics;
        if (diagnostics) void refreshDiagnostics();
      }
    };
    window.addEventListener("keydown", onKey);
    void start();
    scheduleHide();

    return () => {
      disposed = true;
      window.removeEventListener("keydown", onKey);
      document.removeEventListener("focusin", onFocusIn, true);
      restoreBackground?.();
      restoreBackground = null;
      popLayer(layer);
      delete document.body.dataset.playerOpen;
      previousFocus?.focus?.();
      reportProgress(true);
      if (hideTimer !== null) window.clearTimeout(hideTimer);
      if (clickTimer !== null) window.clearTimeout(clickTimer);
      if (frameTimer !== null) window.clearTimeout(frameTimer);
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
      if (fullscreen) void getCurrentWindow().setFullscreen(false).catch(() => {});
      if (windowLabel) {
        void mpvEmbedDestroy(windowLabel).catch(() => {});
      }
    };
  });

  async function start() {
    if (disposed) return;
    error = null;
    ended = false;
    playing = false;
    buffering = true;
    firstFrameMs = null;
    windowLabel = getCurrentWindow().label;

    try {
      const status = await mpvEmbedStatus();
      dllStatus = status.available ? "ok" : "missing";
      if (!status.available) {
        throw new Error(status.lastError ?? "libmpv DLLs are missing.");
      }
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
      unlisten = await listen<MpvEvent>(`mpv-event-${windowLabel}`, (event) => {
        handleEvent(event.payload);
      });
      await mpvEmbedInit(windowLabel);
      loadStartedAt = performance.now();
      pendingStart = startSec > 0 ? startSec : 0;
      await mpvEmbedLoad(windowLabel, playbackSource, stream.headers ?? null);
      await mpvEmbedSet(windowLabel, "pause", false);
      armFrameTimer();
      void refreshDiagnostics();
    } catch (e) {
      if (disposed) return;
      const message = e instanceof Error ? e.message : String(e);
      buffering = false;
      if (onFallback) onFallback(message);
      else error = message;
    }
  }

  function armFrameTimer() {
    if (frameTimer !== null) window.clearTimeout(frameTimer);
    frameTimer = window.setTimeout(() => {
      if (firstFrameMs === null && !disposed) {
        const message = "libmpv did not draw a frame within 8 seconds.";
        if (onFallback) onFallback(message);
        else error = message;
      }
    }, 8000);
  }

  function markFirstFrame() {
    if (firstFrameMs !== null) return;
    firstFrameMs = Math.round(performance.now() - loadStartedAt);
    buffering = false;
    if (frameTimer !== null) {
      window.clearTimeout(frameTimer);
      frameTimer = null;
    }
    console.info(`[mpv] first frame ${firstFrameMs} ms`);
  }

  function handleEvent(payload: MpvEvent) {
    if (!payload) return;
    if (payload.event === "property-change") {
      const name = payload.name;
      const data = payload.data;
      if (name === "time-pos" && typeof data === "number") {
        currentTime = data;
        if (data > 0) markFirstFrame();
        reportProgress(false);
      } else if (name === "duration" && typeof data === "number") {
        duration = data;
      } else if (name === "pause" && typeof data === "boolean") {
        playing = !data;
      } else if (name === "eof-reached" && data === true) {
        finish();
      } else if (name === "hwdec-current" && typeof data === "string") {
        hwdec = data;
      } else if (name === "video-codec" && typeof data === "string") {
        videoCodec = data;
      } else if (name === "video-format" && typeof data === "string") {
        videoFormat = data;
      }
      return;
    }
    if (payload.event === "file-loaded") {
      buffering = false;
      // Resume after the file exists. Setting the mpv start property at
      // runtime is rejected, so seek on the file-loaded edge instead.
      if (pendingStart > 0) {
        const target = pendingStart;
        pendingStart = 0;
        void seek(target);
      }
      void refreshDiagnostics();
      return;
    }
    if (payload.event === "playback-restart") {
      buffering = false;
      return;
    }
    if (payload.event === "end-file") {
      console.info(`[mpv] end-file reason=${payload.reason ?? "none"}`);
      // A reload tears down the previous instance and emits quit on the same
      // window label. Ignore that and normal stops so the new page stays clean.
      if (payload.reason === "eof") finish();
      else if (
        payload.reason !== "stop" &&
        payload.reason !== "redirect" &&
        payload.reason !== "quit"
      ) {
        buffering = false;
        error = "The stream ended before the file finished.";
      }
    }
  }

  async function refreshDiagnostics() {
    if (!windowLabel) return;
    const [hw, codec, format] = await Promise.allSettled([
      mpvEmbedGet(windowLabel, "hwdec-current"),
      mpvEmbedGet(windowLabel, "video-codec"),
      mpvEmbedGet(windowLabel, "video-format"),
    ]);
    if (hw.status === "fulfilled" && typeof hw.value === "string" && hw.value) {
      hwdec = hw.value;
    }
    if (codec.status === "fulfilled" && typeof codec.value === "string" && codec.value) {
      videoCodec = codec.value;
    }
    if (format.status === "fulfilled" && typeof format.value === "string" && format.value) {
      videoFormat = format.value;
    }
  }

  function reportProgress(force: boolean, finished = false) {
    if (!onProgress) return;
    const now = Date.now();
    if (!force && now - lastReport < 5000) return;
    if (duration <= 0 && !finished) return;
    lastReport = now;
    const position = finished ? duration : currentTime;
    const watched = finished || (duration > 0 && position / duration >= 0.9);
    onProgress(position, duration, watched);
  }

  function finish() {
    if (ended) return;
    ended = true;
    playing = false;
    buffering = false;
    showControls();
    reportProgress(true, true);
  }

  function closePlayer() {
    reportProgress(true);
    onClose();
  }

  function togglePlay() {
    if (ended) {
      ended = false;
      void seek(0);
      void setPause(false);
      return;
    }
    const nextPaused = playing;
    playing = !nextPaused;
    void setPause(nextPaused);
  }

  async function setPause(paused: boolean) {
    try {
      await mpvEmbedSet(windowLabel, "pause", paused);
    } catch (e) {
      notify(errorText(e), "error");
    }
  }

  async function seek(value: number) {
    if (!Number.isFinite(value)) return;
    const target = Math.min(Math.max(value, 0), duration || value);
    currentTime = target;
    try {
      await mpvEmbedSeek(windowLabel, target, "absolute");
    } catch (e) {
      notify(errorText(e), "error");
    }
  }

  async function skip(delta: number) {
    const target = Math.max(0, currentTime + delta);
    currentTime = target;
    try {
      await mpvEmbedSeek(windowLabel, delta, "relative");
    } catch (e) {
      notify(errorText(e), "error");
    }
  }

  async function toggleMute() {
    muted = !muted;
    try {
      await mpvEmbedSet(windowLabel, "mute", muted);
    } catch (e) {
      notify(errorText(e), "error");
    }
  }

  async function changeVolume(value: number) {
    volume = Math.min(1, Math.max(0, value));
    try {
      await mpvEmbedSet(windowLabel, "volume", volume * 100);
      if (volume > 0 && muted) {
        muted = false;
        await mpvEmbedSet(windowLabel, "mute", false);
      }
    } catch (e) {
      notify(errorText(e), "error");
    }
  }

  async function setSpeed(value: number) {
    speed = value;
    try {
      await mpvEmbedSet(windowLabel, "speed", value);
    } catch (e) {
      notify(errorText(e), "error");
    }
  }

  async function stepSpeed(direction: 1 | -1) {
    const steps = [0.5, 0.75, 1, 1.25, 1.5, 2];
    const current = steps.indexOf(speed);
    const next = Math.min(steps.length - 1, Math.max(0, (current === -1 ? 2 : current) + direction));
    await setSpeed(steps[next]);
  }

  async function resetSpeed() {
    await setSpeed(1);
  }

  async function cycleSubtitles() {
    try {
      await mpvEmbedCycleSub(windowLabel);
      subtitleState = "cycled";
      notify("Subtitle track cycled.", "info");
    } catch (e) {
      notify(errorText(e), "error");
    }
  }

  async function toggleFullscreen() {
    try {
      const next = !fullscreen;
      await getCurrentWindow().setFullscreen(next);
      fullscreen = next;
    } catch {
      notify("Fullscreen was refused by the window.", "error");
    }
  }

  function onVideoClick() {
    if (clickTimer !== null) {
      window.clearTimeout(clickTimer);
      clickTimer = null;
      return;
    }
    clickTimer = window.setTimeout(() => {
      clickTimer = null;
      togglePlay();
    }, 280);
  }

  function onVideoDoubleClick() {
    if (clickTimer !== null) {
      window.clearTimeout(clickTimer);
      clickTimer = null;
    }
    void toggleFullscreen();
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

  function errorText(value: unknown): string {
    return value instanceof Error ? value.message : String(value);
  }

  function trapTab(event: KeyboardEvent) {
    if (shell && isTopLayer(layer)) trapTabKey(event, shell);
  }

  function controlsHaveFocus(): boolean {
    const active = document.activeElement as HTMLElement | null;
    if (!active || !shell?.contains(active) || active === shell) return false;
    return active.matches("button, input, select, textarea, a[href], [tabindex='0']");
  }

  function showControls() {
    controlsVisible = true;
    scheduleHide();
  }

  function scheduleHide() {
    if (hideTimer !== null) window.clearTimeout(hideTimer);
    hideTimer = window.setTimeout(() => {
      if (playing && !controlsHaveFocus()) controlsVisible = false;
    }, 2500);
  }

  function toggleDiagnostics() {
    diagnostics = !diagnostics;
    if (diagnostics) void refreshDiagnostics();
  }

  async function copyDiagnostics() {
    const lines = [
      "Backend: libmpv",
      `DLLs: ${dllStatus}`,
      `First frame: ${firstFrameMs === null ? "not measured" : `${firstFrameMs} ms`}`,
      `HWDec: ${hwdec}`,
      `Video: ${videoCodec} / ${videoFormat}`,
      `State: ${error ? "error" : ended ? "ended" : playing ? "playing" : "paused"}`,
      `Stream: ${stream.type}${stream.quality ? ` / ${stream.quality}` : ""}`,
      `Host: ${streamHost}`,
      `Time: ${formatTime(currentTime)} / ${formatTime(duration)}`,
      `Speed: ${speed}x`,
      `Subtitles: ${subtitleState}`,
      `Headers: ${hasHeaders ? "requested" : "none"}`,
      `Proxied: ${proxied ? "yes" : "no"}`,
    ];
    try {
      await navigator.clipboard.writeText(lines.join("\n"));
      notify("Diagnostics copied.", "success");
    } catch {
      notify("Could not copy diagnostics.", "error");
    }
  }

  const seekPercent = $derived(duration > 0 ? Math.min(100, (currentTime / duration) * 100) : 0);

  $effect(() => {
    let frame = 0;
    if (error) frame = requestAnimationFrame(() => retryButton?.focus());
    else if (ended) frame = requestAnimationFrame(() => replayButton?.focus());
    return () => cancelAnimationFrame(frame);
  });
</script>

<div
  bind:this={shell}
  class="player-shell mpv-shell fixed inset-0 z-50 flex flex-col"
  role="dialog"
  aria-modal="true"
  aria-labelledby="mpv-player-title"
  aria-describedby={subtitle ? "mpv-player-subtitle" : undefined}
  tabindex={-1}
  data-first-frame={firstFrameMs ?? ""}
  data-hwdec={hwdec}
  data-video-codec={videoCodec}
  data-video-format={videoFormat}
  data-mpv-dlls={dllStatus}
  data-mpv-state={error ? "error" : ended ? "ended" : playing ? "playing" : "paused"}
  onpointermove={showControls}
  onpointerdown={showControls}
  onfocusin={showControls}
  onkeydown={trapTab}
>
  <button
    class="absolute inset-0 w-full h-full cursor-default"
    aria-label="Play or pause"
    onclick={onVideoClick}
    ondblclick={onVideoDoubleClick}
  ></button>

  <div
    class="absolute inset-x-0 top-0 p-3 md:p-4 flex items-start gap-2.5 transition-opacity duration-200 z-10"
    style="background: linear-gradient(0deg, transparent, rgb(0 0 0 / 0.7)); opacity: {controlsVisible ? 1 : 0}; pointer-events: {controlsVisible ? 'auto' : 'none'};"
  >
    <button class="player-glass rounded-xl px-3 py-2 text-xs flex items-center gap-1.5" onclick={closePlayer} aria-label="Close player">
      <Icon name="arrowLeft" size={14} />
      Back
    </button>
    <div class="player-glass rounded-xl px-3.5 py-2 min-w-0 max-w-[52vw]">
      <p id="mpv-player-title" class="text-sm font-semibold truncate">{title}</p>
      {#if subtitle}
        <p id="mpv-player-subtitle" class="text-[11px] truncate mt-0.5" style="color: var(--paper-300);">{subtitle}</p>
      {/if}
    </div>
    <div class="ml-auto flex items-center gap-2">
      <span class="chip text-[10px] px-2 py-0.5 hidden sm:inline-flex">libmpv</span>
      {#if incognito}
        <span class="chip text-[10px] px-2 py-0.5" title="Playback is not saved while Incognito is on">
          <Icon name="eyeOff" size={10} />
          Incognito
        </span>
      {/if}
      {#if hasHeaders}
        <span class="chip text-[10px] px-2 py-0.5 hidden sm:inline-flex">
          <Icon name="shield" size={10} />
          Headers
        </span>
      {/if}
      <button class="player-glass rounded-xl px-3 py-2 text-xs flex items-center gap-1.5" onclick={onOpenExternal} aria-label="Open in mpv">
        <Icon name="external" size={14} />
        <span class="hidden sm:inline">Open in mpv</span>
      </button>
    </div>
  </div>

  {#if error}
    <div class="absolute inset-0 grid place-items-center p-6 z-20" style="background: rgb(0 0 0 / 0.72);">
      <div class="player-glass max-w-md p-5 rounded-2xl" role="alert" aria-labelledby="mpv-player-error-title">
        <span class="w-10 h-10 grid place-items-center rounded-xl" style="background: var(--danger-wash); color: var(--danger);">
          <Icon name="alert" size={20} />
        </span>
        <h2 id="mpv-player-error-title" class="text-lg font-semibold mt-3">Playback stopped</h2>
        <p class="text-sm mt-2 leading-relaxed" style="color: var(--paper-300);">{error}</p>
        <div class="flex flex-wrap gap-2 mt-4">
          <button bind:this={retryButton} class="btn-primary px-3.5 py-2 text-sm" onclick={() => void start()}>
            <Icon name="refresh" size={14} />
            Retry
          </button>
          <button class="btn-ghost px-3.5 py-2 text-sm" onclick={onOpenExternal}>
            <Icon name="external" size={14} />
            Open in mpv
          </button>
          <button class="btn-ghost px-3.5 py-2 text-sm" onclick={() => void copyDiagnostics()}>
            <Icon name="copy" size={14} />
            Copy diagnostics
          </button>
        </div>
      </div>
    </div>
  {:else if ended}
    <p class="sr-only" role="status" aria-live="polite">Playback finished</p>
    <div class="absolute inset-0 grid place-items-center p-6 z-20" style="background: rgb(0 0 0 / 0.62);">
      <div class="player-glass max-w-sm w-full p-5 rounded-2xl text-center">
        <span class="w-12 h-12 grid place-items-center rounded-full mx-auto" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
          <Icon name="check" size={22} />
        </span>
        <p class="eyebrow mt-3">Ended</p>
        <h2 class="text-xl font-semibold mt-1">Playback finished</h2>
        <p class="text-sm mt-2" style="color: var(--paper-300);">{title}</p>
        <div class="flex justify-center gap-2 mt-5">
          <button bind:this={replayButton} class="btn-primary px-4 py-2 text-sm" onclick={togglePlay}>
            <Icon name="refresh" size={14} />
            Replay
          </button>
          <button class="btn-ghost px-4 py-2 text-sm" onclick={closePlayer}>Close</button>
        </div>
      </div>
    </div>
  {:else if buffering}
    <p class="sr-only" role="status" aria-live="polite">Buffering</p>
    <div class="absolute inset-0 grid place-items-center pointer-events-none z-20">
      <div class="player-glass px-5 py-4 rounded-2xl flex flex-col items-center gap-2.5">
        <span class="w-8 h-8 rounded-full border-2 border-t-transparent animate-spin" style="border-color: var(--volt-400); border-top-color: transparent;"></span>
        <span class="text-[10px] mono tracking-[0.12em]" style="color: var(--paper-300);">
          LIBMPV{stream.quality ? ` · ${stream.quality}` : ""}
        </span>
      </div>
    </div>
  {:else if !playing}
    <button class="absolute inset-0 grid place-items-center z-20" onclick={togglePlay} aria-label="Play">
      <span class="player-glass h-20 w-20 grid place-items-center rounded-full" style="box-shadow: 0 0 60px var(--accent-wash-strong);">
        <Icon name="play" size={30} />
      </span>
    </button>
  {/if}

  {#if diagnostics}
    <div class="absolute right-3 md:right-4 top-[76px] z-30 w-[340px] max-w-[calc(100vw-24px)] player-glass rounded-2xl p-4">
      <div class="flex items-center gap-2 mb-3">
        <span style="color: var(--volt-300);"><Icon name="bug" size={15} /></span>
        <h2 class="text-sm font-semibold flex-1">Diagnostics</h2>
        <button class="w-6 h-6 grid place-items-center rounded-md" style="color: var(--paper-500);" onclick={() => (diagnostics = false)} aria-label="Close diagnostics">
          <Icon name="close" size={13} />
        </button>
      </div>
      <dl class="space-y-1.5 text-[11px]">
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Backend</dt><dd class="mono">libmpv</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">DLLs</dt><dd class="mono">{dllStatus}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">First frame</dt><dd class="mono">{firstFrameMs === null ? "..." : `${firstFrameMs} ms`}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">HWDec</dt><dd class="mono truncate max-w-[180px]">{hwdec}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Video</dt><dd class="mono truncate max-w-[180px]">{videoCodec} / {videoFormat}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">State</dt><dd>{error ? "error" : ended ? "ended" : playing ? "playing" : "paused"}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Host</dt><dd class="mono truncate max-w-[180px]">{streamHost}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Time</dt><dd class="mono">{formatTime(currentTime)} / {formatTime(duration)}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Speed</dt><dd class="mono">{speed}x</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Subtitles</dt><dd>{subtitleState}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Headers</dt><dd>{hasHeaders ? "requested" : "none"}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Proxied</dt><dd>{proxied ? "yes" : "no"}</dd></div>
      </dl>
      <button class="btn-ghost w-full mt-3 py-1.5 text-[11px]" onclick={() => void copyDiagnostics()}>
        <Icon name="copy" size={12} />
        Copy diagnostics
      </button>
    </div>
  {/if}

  <div
    class="absolute inset-x-0 bottom-0 p-3 md:p-4 transition-opacity duration-200 z-10"
    style="background: linear-gradient(0deg, rgb(0 0 0 / 0.86), transparent); opacity: {controlsVisible ? 1 : 0}; pointer-events: {controlsVisible ? 'auto' : 'none'};"
  >
    <div class="player-glass rounded-2xl px-3 pt-2.5 pb-2.5">
      <div class="flex items-center gap-2">
        <span class="text-[11px] mono w-12 text-right shrink-0" style="color: var(--paper-300);">{formatTime(currentTime)}</span>
        <input
          type="range"
          min="0"
          max={duration || 0}
          step="0.1"
          value={currentTime}
          oninput={(event) => void seek(Number((event.currentTarget as HTMLInputElement).value))}
          class="seek-volt flex-1"
          style="--seek-progress: {seekPercent}%; --seek-buffer: 0%;"
          aria-label="Seek"
          aria-valuetext={formatTime(currentTime)}
        />
        <span class="text-[11px] mono w-12 shrink-0" style="color: var(--paper-300);">{formatTime(duration)}</span>
      </div>
      <div class="flex items-center gap-1 mt-1.5">
        <button class="player-control" onclick={togglePlay} aria-label={playing ? "Pause" : "Play"}>
          <Icon name={playing ? "pause" : "play"} size={17} />
        </button>
        <button class="player-control" onclick={() => void skip(-10)} aria-label="Back 10 seconds">
          <Icon name="skipBack" size={16} />
        </button>
        <button class="player-control" onclick={() => void skip(10)} aria-label="Forward 10 seconds">
          <Icon name="skipForward" size={16} />
        </button>
        <span class="text-[11px] mono ml-1 hidden sm:inline" style="color: var(--paper-500);">
          {formatTime(currentTime)} / {formatTime(duration)}
        </span>
        <div class="flex items-center gap-1 ml-1.5">
          <button class="player-control" onclick={() => void toggleMute()} aria-label={muted ? "Unmute" : "Mute"}>
            <Icon name={muted || volume === 0 ? "mute" : "volume"} size={17} />
          </button>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={volume}
            oninput={(event) => void changeVolume(Number((event.currentTarget as HTMLInputElement).value))}
            class="range-volt w-20 hidden sm:block"
            style="--range-progress: {volume * 100}%;"
            aria-label="Volume"
            aria-valuetext="{Math.round(volume * 100)} percent"
          />
        </div>
        <div class="ml-auto flex items-center gap-1">
          {#if qualityOptions.length > 1 && onQualityChange}
            <div class="relative">
              <button
                class="player-control {qualityMenuOpen ? 'is-on' : ''}"
                onclick={() => (qualityMenuOpen = !qualityMenuOpen)}
                aria-label="Stream quality"
                aria-haspopup="dialog"
                aria-expanded={qualityMenuOpen}
                title="Stream quality"
                data-quality-toggle
              >
                <Icon name="settings" size={17} />
              </button>
              {#if qualityMenuOpen}
                <PlayerQualityMenu
                  options={qualityOptions}
                  activeUrl={stream.url}
                  onSelect={chooseQuality}
                  onClose={() => (qualityMenuOpen = false)}
                />
              {/if}
            </div>
          {/if}
          <select
            class="player-select"
            value={String(speed)}
            onchange={(event) => void setSpeed(Number((event.currentTarget as HTMLSelectElement).value))}
            aria-label="Playback speed"
            title="Playback speed (, and .)"
          >
            <option value="0.5">0.5x</option>
            <option value="0.75">0.75x</option>
            <option value="1">1x</option>
            <option value="1.25">1.25x</option>
            <option value="1.5">1.5x</option>
            <option value="2">2x</option>
          </select>
          {#if subtitles.length > 0}
            <button class="player-control" onclick={() => void cycleSubtitles()} aria-label="Cycle subtitles" title="Subtitles (V)">
              <Icon name="captions" size={17} />
            </button>
          {/if}
          <button class="player-control {diagnostics ? 'is-on' : ''}" onclick={toggleDiagnostics} aria-label="Diagnostics" aria-pressed={diagnostics} title="Diagnostics (D)">
            <Icon name="bug" size={17} />
          </button>
          <button class="player-control" onclick={() => void toggleFullscreen()} aria-label="Fullscreen" aria-pressed={fullscreen} title="Fullscreen (F)">
            <Icon name="fullscreen" size={17} />
          </button>
        </div>
      </div>
    </div>
  </div>

  <p class="sr-only" role="status" aria-live="polite">{playing ? "Playing" : "Paused"}</p>
</div>

<style>
  .mpv-shell {
    background: transparent;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
  }
</style>
