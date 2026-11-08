<script lang="ts">
  import { onMount } from "svelte";
  import type HlsType from "hls.js";
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
  let volume = $state(0.9);
  let muted = $state(false);
  let speed = $state(1);
  let error = $state<string | null>(null);
  let controlsVisible = $state(true);
  let fullscreen = $state(false);
  let hideTimer: number | null = null;
  let lastReport = 0;

  const subtitles = $derived(stream.subtitles ?? []);
  const hasHeaders = $derived(Object.keys(stream.headers ?? {}).length > 0);

  onMount(() => {
    if (!video) return;
    const element = video;
    element.volume = volume;

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
    const onPlay = () => {
      playing = true;
      scheduleHide();
    };
    const onPause = () => {
      playing = false;
      showControls();
      reportProgress(true);
    };
    const onWaiting = () => {
      buffering = true;
    };
    const onPlaying = () => {
      buffering = false;
      error = null;
    };
    const onEnded = () => {
      playing = false;
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
      } else if (event.key === " " || event.key.toLowerCase() === "k") {
        event.preventDefault();
        togglePlay();
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        seek(currentTime - 10);
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        seek(currentTime + 10);
      } else if (event.key.toLowerCase() === "m") {
        event.preventDefault();
        toggleMute();
      } else if (event.key.toLowerCase() === "f") {
        event.preventDefault();
        void toggleFullscreen();
      }
    };
    window.addEventListener("keydown", onKey);

    void setup();
    scheduleHide();

    return () => {
      element.removeEventListener("timeupdate", onTime);
      element.removeEventListener("loadedmetadata", onDuration);
      element.removeEventListener("play", onPlay);
      element.removeEventListener("pause", onPause);
      element.removeEventListener("waiting", onWaiting);
      element.removeEventListener("playing", onPlaying);
      element.removeEventListener("ended", onEnded);
      element.removeEventListener("error", onError);
      element.removeEventListener("volumechange", onVolume);
      window.removeEventListener("keydown", onKey);
      reportProgress(true);
      if (hideTimer !== null) window.clearTimeout(hideTimer);
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
    buffering = true;
    const isHls = stream.type === "hls" || /\.m3u8(\?|$)/i.test(stream.url);
    if (!isHls) {
      video.src = stream.url;
    } else {
      try {
        const { default: Hls } = await import("hls.js");
        if (!video) return;
        if (Hls.isSupported()) {
          hls = new Hls({ enableWorker: true });
          hls.loadSource(stream.url);
          hls.attachMedia(video);
          hls.on(Hls.Events.ERROR, (_event, data) => {
            if (data.fatal) {
              buffering = false;
              error = "HLS playback failed. Open it in mpv.";
            }
          });
        } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
          video.src = stream.url;
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
</script>

<div
  bind:this={shell}
  class="fixed inset-0 z-50 flex flex-col"
  style="background: #000;"
  role="dialog"
  aria-modal="true"
  aria-label="Video player"
  tabindex={-1}
  onpointermove={showControls}
  onpointerdown={showControls}
>
  <video
    bind:this={video}
    class="absolute inset-0 w-full h-full object-contain"
    playsinline
    onclick={togglePlay}
    ondblclick={() => void toggleFullscreen()}
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
    class="absolute inset-x-0 top-0 p-4 flex items-start gap-3 transition-opacity duration-200"
    style="background: linear-gradient(0deg, transparent, rgb(0 0 0 / 0.72)); opacity: {controlsVisible ? 1 : 0}; pointer-events: {controlsVisible ? 'auto' : 'none'};"
  >
    <button class="btn-ghost px-3 py-1.5 text-xs" onclick={closePlayer} aria-label="Close player">
      ← Back
    </button>
    <div class="min-w-0">
      <p class="text-sm font-semibold truncate">{title}</p>
      {#if subtitle}
        <p class="text-xs truncate" style="color: var(--paper-300);">{subtitle}</p>
      {/if}
    </div>
    <div class="ml-auto flex items-center gap-2">
      {#if hasHeaders}
        <span class="text-[10px] px-2 py-0.5 rounded-full" style="background: rgb(8 8 15 / 0.7); color: var(--paper-300);">
          Headers requested
        </span>
      {/if}
      <button class="btn-ghost px-3 py-1.5 text-xs" onclick={onOpenExternal}>
        Open in mpv
      </button>
    </div>
  </div>

  {#if error}
    <div class="absolute inset-0 grid place-items-center p-6" style="background: rgb(0 0 0 / 0.72);">
      <div class="max-w-md p-5 rounded-2xl" style="background: var(--void-900); border: 1px solid var(--line);">
        <h2 class="text-lg font-semibold">Playback stopped</h2>
        <p class="text-sm mt-2" style="color: var(--paper-300);">{error}</p>
        <div class="flex gap-2 mt-4">
          <button class="btn-primary px-3 py-1.5 text-sm" onclick={retry}>Retry</button>
          <button class="btn-ghost px-3 py-1.5 text-sm" onclick={onOpenExternal}>Open in mpv</button>
        </div>
      </div>
    </div>
  {:else if buffering}
    <div class="absolute inset-0 grid place-items-center pointer-events-none">
      <div class="h-10 w-10 rounded-full border-2 border-t-transparent animate-spin" style="border-color: var(--volt-400); border-top-color: transparent;"></div>
    </div>
  {:else if !playing}
    <button
      class="absolute inset-0 grid place-items-center"
      onclick={togglePlay}
      aria-label="Play"
    >
      <span class="h-20 w-20 grid place-items-center rounded-full" style="background: rgb(8 8 15 / 0.7); border: 1px solid var(--line); backdrop-filter: blur(12px);">
        <svg width="30" height="30" viewBox="0 0 24 24" fill="var(--paper-50)" aria-hidden="true">
          <path d="M8 5v14l11-7z" />
        </svg>
      </span>
    </button>
  {/if}

  <div
    class="absolute inset-x-0 bottom-0 p-3 transition-opacity duration-200"
    style="background: linear-gradient(0deg, rgb(0 0 0 / 0.86), transparent); opacity: {controlsVisible ? 1 : 0}; pointer-events: {controlsVisible ? 'auto' : 'none'};"
  >
    <input
      type="range"
      min="0"
      max={duration || 0}
      step="0.1"
      value={currentTime}
      oninput={(event) => seek(Number((event.currentTarget as HTMLInputElement).value))}
      class="w-full h-1 accent-[var(--volt-400)] cursor-pointer"
      aria-label="Seek"
    />
    <div class="flex items-center gap-3 mt-2">
      <button class="w-9 h-9 grid place-items-center rounded-lg" onclick={togglePlay} aria-label={playing ? "Pause" : "Play"}>
        {#if playing}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="var(--paper-50)" aria-hidden="true"><path d="M7 5h4v14H7zM13 5h4v14h-4z" /></svg>
        {:else}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="var(--paper-50)" aria-hidden="true"><path d="M8 5v14l11-7z" /></svg>
        {/if}
      </button>
      <span class="text-xs" style="color: var(--paper-300); font-family: var(--font-mono);">
        {formatTime(currentTime)} / {formatTime(duration)}
      </span>
      <div class="flex items-center gap-1.5 ml-2">
        <button class="w-8 h-8 grid place-items-center rounded-lg" onclick={toggleMute} aria-label={muted ? "Unmute" : "Mute"}>
          {#if muted || volume === 0}
            <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="var(--paper-50)" stroke-width="1.5" aria-hidden="true"><path d="M4 9v6h4l5 4V5L8 9H4z" /><path d="M17 9l4 6M21 9l-4 6" /></svg>
          {:else}
            <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="var(--paper-50)" stroke-width="1.5" aria-hidden="true"><path d="M4 9v6h4l5 4V5L8 9H4z" /><path d="M16 8a5 5 0 0 1 0 8" /></svg>
          {/if}
        </button>
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          value={volume}
          oninput={(event) => changeVolume(Number((event.currentTarget as HTMLInputElement).value))}
          class="w-20 h-1 accent-[var(--volt-400)] cursor-pointer"
          aria-label="Volume"
        />
      </div>
      <div class="ml-auto flex items-center gap-1.5">
        <select
          bind:value={speed}
          onchange={changeSpeed}
          class="h-8 rounded-lg px-2 text-xs outline-none"
          style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-50);"
          aria-label="Playback speed"
        >
          <option value={0.5}>0.5x</option>
          <option value={0.75}>0.75x</option>
          <option value={1}>1x</option>
          <option value={1.25}>1.25x</option>
          <option value={1.5}>1.5x</option>
          <option value={2}>2x</option>
        </select>
        <button class="w-9 h-9 grid place-items-center rounded-lg" onclick={() => void toggleFullscreen()} aria-label="Fullscreen">
          <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="var(--paper-50)" stroke-width="1.8" aria-hidden="true"><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" /></svg>
        </button>
      </div>
    </div>
  </div>
</div>
