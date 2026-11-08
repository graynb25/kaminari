<script lang="ts">
  import { onMount } from "svelte";
  import type HlsType from "hls.js";
  import Icon from "./Icon.svelte";
  import { playbackUrl } from "./streamProxy";
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
  }: {
    stream: JsStream;
    title: string;
    subtitle?: string;
    startSec?: number;
    onClose: () => void;
    onOpenExternal: () => void;
    notify: (text: string, tone?: "info" | "success" | "error") => void;
    onProgress?: (position: number, duration: number, watched: boolean) => void;
  } = $props();

  let shell: HTMLDivElement | null = $state(null);
  let video: HTMLVideoElement | null = $state(null);
  let hls: HlsType | null = null;
  let playing = $state(false);
  let buffering = $state(true);
  let currentTime = $state(0);
  let duration = $state(0);
  let buffered = $state(0);
  let volume = $state(0.9);
  let muted = $state(false);
  let speed = $state(1);
  let error = $state<string | null>(null);
  let controlsVisible = $state(true);
  let fullscreen = $state(false);
  let ended = $state(false);
  let fill = $state(false);
  let diagnostics = $state(false);
  let activeSubtitle = $state("Off");
  let hideTimer: number | null = null;
  let clickTimer: number | null = null;
  let lastReport = 0;
  let previousFocus: HTMLElement | null = null;

  const subtitles = $derived(stream.subtitles ?? []);
  const hasHeaders = $derived(Object.keys(stream.headers ?? {}).length > 0);
  const playbackSource = $derived(playbackUrl(stream));
  const proxied = $derived(Boolean(stream.proxiedUrl));
  const streamHost = $derived.by(() => {
    try {
      return new URL(stream.url).host;
    } catch {
      return "unknown host";
    }
  });

  onMount(() => {
    if (!video) return;
    const element = video;
    previousFocus = document.activeElement as HTMLElement | null;
    element.volume = volume;
    document.body.dataset.playerOpen = "true";
    shell?.focus();

    const onTime = () => {
      currentTime = element.currentTime;
      reportProgress(false);
    };
    const onDuration = () => {
      duration = Number.isFinite(element.duration) ? element.duration : 0;
      if (startSec > 0 && element.currentTime < startSec) {
        element.currentTime = startSec;
      }
    };
    const onProgress = () => {
      if (element.buffered.length > 0) {
        buffered = element.buffered.end(element.buffered.length - 1);
      }
    };
    const onPlay = () => {
      playing = true;
      ended = false;
      scheduleHide();
    };
    const onPause = () => {
      playing = false;
      showControls();
      reportProgress(true);
    };
    const onWaiting = () => {
      buffering = true;
      showControls();
    };
    const onPlaying = () => {
      buffering = false;
      error = null;
      ended = false;
    };
    const onEnded = () => {
      playing = false;
      ended = true;
      showControls();
      reportProgress(true, true);
    };
    const onError = () => {
      buffering = false;
      const code = element.error?.code;
      error =
        code === 4
          ? "This stream format is not supported in the WebView. Open it in mpv."
          : "The stream stopped. Retry, or open it in mpv.";
    };
    const onVolume = () => {
      volume = element.volume;
      muted = element.muted;
    };

    element.addEventListener("timeupdate", onTime);
    element.addEventListener("progress", onProgress);
    element.addEventListener("loadedmetadata", onDuration);
    element.addEventListener("play", onPlay);
    element.addEventListener("pause", onPause);
    element.addEventListener("waiting", onWaiting);
    element.addEventListener("playing", onPlaying);
    element.addEventListener("ended", onEnded);
    element.addEventListener("error", onError);
    element.addEventListener("volumechange", onVolume);

    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
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
        seek(currentTime - 10);
      } else if (key === "l") {
        event.preventDefault();
        seek(currentTime + 10);
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        seek(currentTime - (event.shiftKey ? 60 : 10));
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        seek(currentTime + (event.shiftKey ? 60 : 10));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        changeVolume(Math.min(1, volume + 0.05));
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        changeVolume(Math.max(0, volume - 0.05));
      } else if (key === "m") {
        event.preventDefault();
        toggleMute();
      } else if (key === "f") {
        event.preventDefault();
        void toggleFullscreen();
      } else if (key === "t") {
        event.preventDefault();
        toggleFill();
      } else if (key === "v") {
        event.preventDefault();
        cycleSubtitles();
      } else if (event.key === ",") {
        event.preventDefault();
        stepSpeed(-1);
      } else if (event.key === ".") {
        event.preventDefault();
        stepSpeed(1);
      } else if (event.key === "0") {
        event.preventDefault();
        resetSpeed();
      } else if (key === "d") {
        event.preventDefault();
        diagnostics = !diagnostics;
      }
    };
    window.addEventListener("keydown", onKey);

    void setup();
    scheduleHide();

    return () => {
      element.removeEventListener("timeupdate", onTime);
      element.removeEventListener("progress", onProgress);
      element.removeEventListener("loadedmetadata", onDuration);
      element.removeEventListener("play", onPlay);
      element.removeEventListener("pause", onPause);
      element.removeEventListener("waiting", onWaiting);
      element.removeEventListener("playing", onPlaying);
      element.removeEventListener("ended", onEnded);
      element.removeEventListener("error", onError);
      element.removeEventListener("volumechange", onVolume);
      window.removeEventListener("keydown", onKey);
      delete document.body.dataset.playerOpen;
      previousFocus?.focus?.();
      reportProgress(true);
      if (hideTimer !== null) window.clearTimeout(hideTimer);
      if (clickTimer !== null) window.clearTimeout(clickTimer);
      hls?.destroy();
      hls = null;
      element.pause();
      element.removeAttribute("src");
      element.load();
    };
  });

  function reportProgress(force: boolean, ended = false) {
    if (!video || !onProgress) return;
    const now = Date.now();
    if (!force && now - lastReport < 5000) return;
    lastReport = now;
    const total = Number.isFinite(video.duration) ? video.duration : 0;
    const position = ended ? total : video.currentTime;
    const watched = ended || (total > 0 && position / total >= 0.9);
    onProgress(position, total, watched);
  }

  function closePlayer() {
    reportProgress(true);
    onClose();
  }

  async function setup() {
    if (!video) return;
    error = null;
    ended = false;
    buffering = true;
    const isHls = stream.type === "hls" || /\.m3u8(\?|$)/i.test(playbackSource);
    if (!isHls) {
      video.src = playbackSource;
    } else {
      try {
        const { default: Hls } = await import("hls.js");
        if (!video) return;
        if (Hls.isSupported()) {
          hls = new Hls({ enableWorker: true });
          hls.loadSource(playbackSource);
          hls.attachMedia(video);
          hls.on(Hls.Events.ERROR, (_event, data) => {
            if (data.fatal) {
              buffering = false;
              error = "HLS playback failed. Open it in mpv.";
            }
          });
        } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
          video.src = playbackSource;
        } else {
          buffering = false;
          error = "HLS is not supported in this WebView. Open it in mpv.";
          return;
        }
      } catch {
        buffering = false;
        error = "Could not load the HLS engine. Open it in mpv.";
        return;
      }
    }
    try {
      await video.play();
    } catch {
      // Autoplay can be blocked. The center play button stays visible.
      buffering = false;
    }
  }

  function togglePlay() {
    if (!video) return;
    if (ended) {
      ended = false;
      seek(0);
      void video.play();
      return;
    }
    if (video.paused) void video.play();
    else video.pause();
  }

  function seek(value: number) {
    if (!video || !Number.isFinite(value)) return;
    video.currentTime = Math.min(Math.max(value, 0), duration || value);
    currentTime = video.currentTime;
  }

  function toggleMute() {
    if (!video) return;
    video.muted = !video.muted;
  }

  function changeVolume(value: number) {
    if (!video) return;
    video.volume = value;
    if (value > 0) video.muted = false;
  }

  function changeSpeed() {
    if (video) video.playbackRate = speed;
  }

  function stepSpeed(direction: 1 | -1) {
    const steps = [0.5, 0.75, 1, 1.25, 1.5, 2];
    const current = steps.indexOf(speed);
    const next = Math.min(steps.length - 1, Math.max(0, (current === -1 ? 2 : current) + direction));
    speed = steps[next];
    changeSpeed();
  }

  function resetSpeed() {
    speed = 1;
    changeSpeed();
  }

  function cycleSubtitles() {
    if (!video) return;
    const tracks = video.textTracks;
    if (tracks.length === 0) {
      notify("This stream has no subtitle tracks.", "info");
      return;
    }
    let current = -1;
    for (let index = 0; index < tracks.length; index += 1) {
      if (tracks[index].mode === "showing") current = index;
      tracks[index].mode = "disabled";
    }
    const next = current + 1;
    if (next >= tracks.length) {
      activeSubtitle = "Off";
      notify("Subtitles off.", "info");
      return;
    }
    tracks[next].mode = "showing";
    activeSubtitle = tracks[next].label || tracks[next].language || `Track ${next + 1}`;
    notify(`Subtitles: ${activeSubtitle}`, "info");
  }

  function toggleFill() {
    fill = !fill;
  }

  function toggleDiagnostics() {
    diagnostics = !diagnostics;
  }

  async function toggleFullscreen() {
    if (!shell) return;
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
        fullscreen = false;
      } else {
        await shell.requestFullscreen();
        fullscreen = true;
      }
    } catch {
      notify("Fullscreen was refused by the window.", "error");
    }
  }

  function trapTab(event: KeyboardEvent) {
    if (event.key !== "Tab" || !shell) return;
    const items = [...shell.querySelectorAll("button, input, select")].filter(
      (el) => (el as HTMLElement).tabIndex >= 0 && !(el as HTMLButtonElement).disabled,
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

  function showControls() {
    controlsVisible = true;
    scheduleHide();
  }

  function scheduleHide() {
    if (hideTimer !== null) window.clearTimeout(hideTimer);
    hideTimer = window.setTimeout(() => {
      if (playing) controlsVisible = false;
    }, 2500);
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

  function retry() {
    if (!video) return;
    error = null;
    void setup();
  }

  async function copyDiagnostics() {
    const lines = [
      `Title: ${title}`,
      subtitle ? `Episode: ${subtitle}` : "",
      `State: ${error ? "error" : ended ? "ended" : playing ? "playing" : "paused"}`,
      `Stream: ${stream.type}${stream.quality ? ` / ${stream.quality}` : ""}`,
      `Host: ${streamHost}`,
      `Time: ${formatTime(currentTime)} / ${formatTime(duration)}`,
      `Speed: ${speed}x`,
      `Fill: ${fill ? "on" : "off"}`,
      `Subtitles: ${activeSubtitle}`,
      `Headers: ${hasHeaders ? "requested" : "none"}`,
      `Proxied: ${proxied ? "yes" : "no"}`,
    ].filter(Boolean);
    try {
      await navigator.clipboard.writeText(lines.join("\n"));
      notify("Diagnostics copied.", "success");
    } catch {
      notify("Could not copy diagnostics.", "error");
    }
  }

  const seekPercent = $derived(duration > 0 ? Math.min(100, (currentTime / duration) * 100) : 0);
  const bufferPercent = $derived(duration > 0 ? Math.min(100, (buffered / duration) * 100) : 0);
</script>

<div
  bind:this={shell}
  class="player-shell fixed inset-0 z-50 flex flex-col"
  role="dialog"
  aria-modal="true"
  aria-label="Video player"
  tabindex={-1}
  onpointermove={showControls}
  onpointerdown={showControls}
  onfocusin={showControls}
  onkeydown={trapTab}
>
  <video
    bind:this={video}
    class="absolute inset-0 w-full h-full {fill ? 'object-cover' : 'object-contain'}"
    playsinline
    onclick={onVideoClick}
    ondblclick={onVideoDoubleClick}
  >
    {#each subtitles as subtitleTrack, index}
      <track
        kind="subtitles"
        src={subtitleTrack.url}
        label={subtitleTrack.label ?? subtitleTrack.lang}
        srclang={subtitleTrack.lang}
        default={index === 0}
      />
    {/each}
  </video>

  <div
    class="absolute inset-x-0 top-0 p-3 md:p-4 flex items-start gap-2.5 transition-opacity duration-200"
    style="background: linear-gradient(0deg, transparent, rgb(0 0 0 / 0.7)); opacity: {controlsVisible ? 1 : 0}; pointer-events: {controlsVisible ? 'auto' : 'none'};"
  >
    <button class="player-glass rounded-xl px-3 py-2 text-xs flex items-center gap-1.5" onclick={closePlayer} aria-label="Close player">
      <Icon name="arrowLeft" size={14} />
      Back
    </button>
    <div class="player-glass rounded-xl px-3.5 py-2 min-w-0 max-w-[52vw]">
      <p class="text-sm font-semibold truncate">{title}</p>
      {#if subtitle}
        <p class="text-[11px] truncate mt-0.5" style="color: var(--paper-300);">{subtitle}</p>
      {/if}
    </div>
    <div class="ml-auto flex items-center gap-2">
      {#if hasHeaders}
        <span class="chip text-[10px] px-2 py-0.5 hidden sm:inline-flex">
          <Icon name="shield" size={10} />
          Headers
        </span>
      {/if}
      <button class="player-glass rounded-xl px-3 py-2 text-xs flex items-center gap-1.5" onclick={onOpenExternal}>
        <Icon name="external" size={14} />
        <span class="hidden sm:inline">Open in mpv</span>
      </button>
    </div>
  </div>

  {#if error}
    <div class="absolute inset-0 grid place-items-center p-6" style="background: rgb(0 0 0 / 0.72);">
      <div class="player-glass max-w-md p-5 rounded-2xl">
        <span class="w-10 h-10 grid place-items-center rounded-xl" style="background: var(--danger-wash); color: var(--danger);">
          <Icon name="alert" size={20} />
        </span>
        <h2 class="text-lg font-semibold mt-3">Playback stopped</h2>
        <p class="text-sm mt-2 leading-relaxed" style="color: var(--paper-300);">{error}</p>
        <div class="flex flex-wrap gap-2 mt-4">
          <button class="btn-primary px-3.5 py-2 text-sm" onclick={retry}>
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
    <div class="absolute inset-0 grid place-items-center p-6" style="background: rgb(0 0 0 / 0.62);">
      <div class="player-glass max-w-sm w-full p-5 rounded-2xl text-center">
        <span class="w-12 h-12 grid place-items-center rounded-full mx-auto" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
          <Icon name="check" size={22} />
        </span>
        <p class="eyebrow mt-3">Ended</p>
        <h2 class="text-xl font-semibold mt-1">Playback finished</h2>
        <p class="text-sm mt-2" style="color: var(--paper-300);">{title}</p>
        <div class="flex justify-center gap-2 mt-5">
          <button class="btn-primary px-4 py-2 text-sm" onclick={togglePlay}>
            <Icon name="refresh" size={14} />
            Replay
          </button>
          <button class="btn-ghost px-4 py-2 text-sm" onclick={closePlayer}>Close</button>
        </div>
      </div>
    </div>
  {:else if buffering}
    <div class="absolute inset-0 grid place-items-center pointer-events-none">
      <div class="player-glass px-5 py-4 rounded-2xl flex flex-col items-center gap-2.5">
        <span class="w-8 h-8 rounded-full border-2 border-t-transparent animate-spin" style="border-color: var(--volt-400); border-top-color: transparent;"></span>
        <span class="text-[10px] mono tracking-[0.12em]" style="color: var(--paper-300);">
          {stream.type.toUpperCase()}{stream.quality ? ` Â· ${stream.quality}` : ""}
        </span>
      </div>
    </div>
  {:else if !playing}
    <button class="absolute inset-0 grid place-items-center" onclick={togglePlay} aria-label="Play">
      <span class="player-glass h-20 w-20 grid place-items-center rounded-full" style="box-shadow: 0 0 60px var(--accent-wash-strong);">
        <Icon name="play" size={30} />
      </span>
    </button>
  {/if}

  {#if diagnostics}
    <div class="absolute right-3 md:right-4 top-[76px] z-20 w-[320px] max-w-[calc(100vw-24px)] player-glass rounded-2xl p-4">
      <div class="flex items-center gap-2 mb-3">
        <span style="color: var(--volt-300);"><Icon name="bug" size={15} /></span>
        <h2 class="text-sm font-semibold flex-1">Diagnostics</h2>
        <button class="w-6 h-6 grid place-items-center rounded-md" style="color: var(--paper-500);" onclick={() => (diagnostics = false)} aria-label="Close diagnostics">
          <Icon name="close" size={13} />
        </button>
      </div>
      <dl class="space-y-1.5 text-[11px]">
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">State</dt><dd>{error ? "error" : ended ? "ended" : playing ? "playing" : "paused"}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Stream</dt><dd class="mono">{stream.type}{stream.quality ? ` / ${stream.quality}` : ""}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Host</dt><dd class="mono truncate max-w-[180px]">{streamHost}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Time</dt><dd class="mono">{formatTime(currentTime)} / {formatTime(duration)}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Speed</dt><dd class="mono">{speed}x</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Fill</dt><dd>{fill ? "on" : "off"}</dd></div>
        <div class="flex justify-between gap-3"><dt style="color: var(--paper-500);">Subtitles</dt><dd>{activeSubtitle}</dd></div>
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
    class="absolute inset-x-0 bottom-0 p-3 md:p-4 transition-opacity duration-200"
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
          oninput={(event) => seek(Number((event.currentTarget as HTMLInputElement).value))}
          class="seek-volt flex-1"
          style="--seek-progress: {seekPercent}%; --seek-buffer: {bufferPercent}%;"
          aria-label="Seek"
        />
        <span class="text-[11px] mono w-12 shrink-0" style="color: var(--paper-300);">{formatTime(duration)}</span>
      </div>
      <div class="flex items-center gap-1 mt-1.5">
        <button class="player-control" onclick={togglePlay} aria-label={playing ? "Pause" : "Play"}>
          <Icon name={playing ? "pause" : "play"} size={17} />
        </button>
        <button class="player-control" onclick={() => seek(currentTime - 10)} aria-label="Back 10 seconds">
          <Icon name="skipBack" size={16} />
        </button>
        <button class="player-control" onclick={() => seek(currentTime + 10)} aria-label="Forward 10 seconds">
          <Icon name="skipForward" size={16} />
        </button>
        <span class="text-[11px] mono ml-1 hidden sm:inline" style="color: var(--paper-500);">
          {formatTime(currentTime)} / {formatTime(duration)}
        </span>
        <div class="flex items-center gap-1 ml-1.5">
          <button class="player-control" onclick={toggleMute} aria-label={muted ? "Unmute" : "Mute"}>
            <Icon name={muted || volume === 0 ? "mute" : "volume"} size={17} />
          </button>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={volume}
            oninput={(event) => changeVolume(Number((event.currentTarget as HTMLInputElement).value))}
            class="range-volt w-20 hidden sm:block"
            style="--range-progress: {volume * 100}%;"
            aria-label="Volume"
          />
        </div>
        <div class="ml-auto flex items-center gap-1">
          <select
            class="player-select"
            value={String(speed)}
            onchange={(event) => {
              speed = Number((event.currentTarget as HTMLSelectElement).value);
              changeSpeed();
            }}
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
            <button class="player-control" onclick={cycleSubtitles} aria-label="Cycle subtitles" title="Subtitles (V)">
              <Icon name="captions" size={17} />
            </button>
          {/if}
          <button class="player-control {fill ? 'is-on' : ''}" onclick={toggleFill} aria-label="Fill frame" title="Fill frame (T)">
            <Icon name="monitor" size={17} />
          </button>
          <button class="player-control {diagnostics ? 'is-on' : ''}" onclick={toggleDiagnostics} aria-label="Diagnostics" title="Diagnostics (D)">
            <Icon name="bug" size={17} />
          </button>
          <button class="player-control" onclick={() => void toggleFullscreen()} aria-label="Fullscreen" title="Fullscreen (F)">
            <Icon name="fullscreen" size={17} />
          </button>
        </div>
      </div>
    </div>
  </div>

  <p class="sr-only" aria-live="polite">{playing ? "Playing" : "Paused"} at {formatTime(currentTime)}</p>
</div>

<style>
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
  }
</style>
