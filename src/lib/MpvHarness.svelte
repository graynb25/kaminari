<script lang="ts">
  // Dev-only entry for the libmpv spike. Open with ?mpvHarness=1.
  // ?file=C:\path\to\clip.mp4 plays a local sample without committing it.
  import { onMount } from "svelte";
  import MpvView from "./MpvView.svelte";
  import PlayerView from "./PlayerView.svelte";
  import { mpvEmbedStatus } from "./api";
  import type { JsStream } from "./types";

  const params = new URLSearchParams(location.search);
  const localFile = params.get("file");
  const sample = params.get("sample") ?? "popeye";
  const samples: Record<string, string> = {
    popeye: "https://archive.org/download/PopeyeAliBaba/PopeyeAliBaba_512kb.mp4",
    short: "https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/720/Big_Buck_Bunny_720_10s_1MB.mp4",
  };
  const url = localFile ?? samples[sample] ?? samples.popeye;
  const stream: JsStream = {
    url,
    type: "mp4",
    quality: localFile ? "local file" : sample,
  };

  let active = $state(true);
  let backend = $state<"mpv" | "web">("mpv");
  let message = $state<string | null>(null);
  let tone = $state<"info" | "success" | "error">("info");

  onMount(() => {
    void (async () => {
      try {
        const status = await mpvEmbedStatus();
        if (!status.available) {
          backend = "web";
          notify(
            `libmpv is unavailable: ${status.lastError ?? "DLLs are missing"}. Using the WebView player.`,
            "info",
          );
        }
      } catch (e) {
        backend = "web";
        notify(`libmpv status failed: ${errorText(e)}. Using the WebView player.`, "error");
      }
    })();
  });

  function notify(text: string, nextTone: "info" | "success" | "error" = "info") {
    message = text;
    tone = nextTone;
  }

  function fallback(reason: string) {
    backend = "web";
    notify(`libmpv stopped: ${reason}. Using the WebView player.`, "error");
  }

  function errorText(value: unknown): string {
    return value instanceof Error ? value.message : String(value);
  }
</script>

{#if active}
  {#if backend === "mpv"}
    <MpvView
      {stream}
      title="libmpv dev harness"
      subtitle={localFile ? "local HEVC sample" : "public domain sample"}
      onClose={() => (active = false)}
      onOpenExternal={() => notify("The harness does not open external mpv.", "info")}
      onFallback={fallback}
      {notify}
    />
  {:else}
    <PlayerView
      {stream}
      title="libmpv dev harness"
      subtitle="WebView fallback"
      onClose={() => (active = false)}
      onOpenExternal={() => notify("The harness does not open external mpv.", "info")}
      {notify}
    />
  {/if}
{:else}
  <div class="fixed inset-0 grid place-items-center" style="background: var(--void-950);">
    <div class="player-glass rounded-2xl p-5 max-w-sm text-center">
      <p class="text-sm font-semibold">libmpv harness closed</p>
      <p class="text-xs mt-2" style="color: var(--paper-300);">
        Start the sample again, or close this window.
      </p>
      <button
        class="btn-primary px-4 py-2 text-sm mt-4"
        onclick={() => {
          backend = "mpv";
          active = true;
        }}
      >
        Reopen sample
      </button>
    </div>
  </div>
{/if}

{#if message}
  <div
    class="fixed top-16 left-1/2 -translate-x-1/2 z-[60] player-glass rounded-xl px-4 py-2.5 text-xs max-w-[calc(100vw-32px)]"
    style="color: {tone === 'error' ? 'var(--danger)' : 'var(--paper-100)'};"
    role="status"
  >
    {message}
  </div>
{/if}
