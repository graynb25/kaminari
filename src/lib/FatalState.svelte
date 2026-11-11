<script lang="ts">
  import { onMount } from "svelte";
  import BrandMark from "./BrandMark.svelte";
  import Icon from "./Icon.svelte";
  import { redactSecrets } from "./redact";

  let {
    code,
    title = "Kaminari could not start",
    message = "The app shell loaded, but the boot snapshot failed. Library and browse are paused until it succeeds.",
    detail = null,
    retryLabel = "Retry",
    onRetry,
  }: {
    code: string;
    title?: string;
    message?: string;
    detail?: string | null;
    retryLabel?: string;
    onRetry?: () => void;
  } = $props();

  let heading: HTMLHeadingElement | null = $state(null);
  let copied = $state(false);

  onMount(() => {
    heading?.focus();
  });

  function diagnosticsText(): string {
    const lines = [
      "Kaminari diagnostics (redacted)",
      `Code: ${code}`,
      `Version: ${__APP_VERSION__}`,
      `Time: ${new Date().toISOString()}`,
      `Message: ${redactSecrets(detail ?? message)}`,
      "Note: tokens and secrets are never included.",
    ];
    return lines.join("\n");
  }

  async function copyDiagnostics() {
    try {
      await navigator.clipboard.writeText(diagnosticsText());
      copied = true;
      window.setTimeout(() => (copied = false), 2000);
    } catch {
      copied = false;
    }
  }
</script>

<div role="alert" class="fatal-wrap">
  <div class="fatal-card surface">
    <BrandMark size={48} />
    <!-- Focusable heading so screen readers land on the error. Programmatic focus only. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <h2 bind:this={heading} tabindex={-1} class="fatal-title">{title}</h2>
    <p class="fatal-body">{message}</p>
    <p class="fatal-code mono" aria-label="Error code">{code}</p>
    {#if detail}
      <p class="fatal-detail">{redactSecrets(detail)}</p>
    {/if}
    <div class="fatal-actions">
      {#if onRetry}
        <button class="btn-primary px-4 py-2.5 text-sm" onclick={onRetry}>
          <Icon name="refresh" size={14} />
          {retryLabel}
        </button>
      {/if}
      <button class="btn-ghost px-4 py-2.5 text-sm" onclick={copyDiagnostics}>
        <Icon name="copy" size={14} />
        {copied ? "Copied" : "Copy diagnostics"}
      </button>
    </div>
    <p class="fatal-note">
      This covers failures after the WebView loads. Database errors before the
      window appears use a native recovery dialog instead.
    </p>
  </div>
</div>

<style>
  .fatal-wrap {
    display: grid;
    place-items: center;
    padding: 48px 24px;
    min-height: 50vh;
  }

  .fatal-card {
    display: grid;
    justify-items: center;
    gap: 10px;
    max-width: 520px;
    width: 100%;
    padding: 32px 28px;
    text-align: center;
  }

  .fatal-title {
    margin-top: 6px;
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--text-1);
  }

  .fatal-title:focus {
    outline: none;
  }

  .fatal-title:focus-visible {
    outline: 2px solid var(--volt-400);
    outline-offset: 4px;
    border-radius: 6px;
  }

  .fatal-body {
    font-size: 14px;
    line-height: 1.6;
    color: var(--text-2);
    max-width: 44ch;
  }

  .fatal-code {
    margin-top: 4px;
    padding: 6px 12px;
    border-radius: 8px;
    background: var(--well-bg);
    border: 1px solid var(--line);
    color: var(--text-2);
    font-size: 12px;
  }

  .fatal-detail {
    font-size: 12px;
    line-height: 1.55;
    color: var(--text-4);
    max-width: 52ch;
    overflow-wrap: anywhere;
  }

  .fatal-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    margin-top: 8px;
  }

  .fatal-note {
    margin-top: 6px;
    font-size: 11px;
    line-height: 1.55;
    color: var(--text-4);
    max-width: 52ch;
  }
</style>
