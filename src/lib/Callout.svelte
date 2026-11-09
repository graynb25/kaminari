<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  type Tone = "info" | "success" | "warning" | "danger";

  let {
    tone = "info",
    title,
    icon,
    role,
    actionLabel,
    actionIcon,
    onAction,
    children,
    class: className = "",
  }: {
    tone?: Tone;
    title?: string;
    icon?: string;
    role?: "status" | "alert";
    actionLabel?: string;
    actionIcon?: string;
    onAction?: () => void;
    children?: Snippet;
    class?: string;
  } = $props();

  const icons: Record<Tone, string> = {
    info: "info",
    success: "checkCircle",
    warning: "alert",
    danger: "alert",
  };

  const iconName = $derived(icon ?? icons[tone]);
  const resolvedRole = $derived(role ?? (tone === "danger" ? "alert" : "status"));
</script>

<div class="callout {className}" data-tone={tone} role={resolvedRole}>
  <span class="callout-icon" aria-hidden="true">
    <Icon name={iconName} size={14} />
  </span>
  <div class="callout-copy">
    {#if title}
      <p class="callout-title">{title}</p>
    {/if}
    {#if children}
      <div class="callout-body">{@render children()}</div>
    {/if}
  </div>
  {#if actionLabel && onAction}
    <button type="button" class="btn-ghost callout-action" onclick={onAction}>
      {#if actionIcon}
        <Icon name={actionIcon} size={12} />
      {/if}
      {actionLabel}
    </button>
  {/if}
</div>

<style>
  .callout {
    --callout-wash: var(--info-wash);
    --callout-line: var(--info-line);
    --callout-tone: var(--info);
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--callout-wash);
    border: 1px solid var(--callout-line);
    color: var(--text-2);
  }

  .callout[data-tone="success"] {
    --callout-wash: var(--success-wash);
    --callout-line: var(--success-line);
    --callout-tone: var(--success);
  }

  .callout[data-tone="warning"] {
    --callout-wash: var(--warning-wash);
    --callout-line: var(--warning-line);
    --callout-tone: var(--warning);
  }

  .callout[data-tone="danger"] {
    --callout-wash: var(--danger-wash);
    --callout-line: var(--danger-line);
    --callout-tone: var(--danger);
  }

  .callout-icon {
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    border-radius: 7px;
    background: var(--callout-wash);
    color: var(--callout-tone);
  }

  .callout-copy {
    flex: 1 1 220px;
    min-width: 0;
    display: grid;
    gap: 3px;
  }

  .callout-title {
    font-size: 13px;
    font-weight: 650;
    color: var(--text-1);
  }

  .callout-body {
    font-size: 13px;
    line-height: 1.55;
  }

  .callout-action {
    margin-left: auto;
    flex-shrink: 0;
    padding: 0.35rem 0.7rem;
    font-size: 12px;
  }
</style>
