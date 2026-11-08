<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    icon,
    title,
    body,
    actionLabel,
    actionIcon = "arrowRight",
    onAction,
    secondaryLabel,
    onSecondary,
    compact = false,
  }: {
    icon: string;
    title: string;
    body: string;
    actionLabel?: string;
    actionIcon?: string;
    onAction?: () => void;
    secondaryLabel?: string;
    onSecondary?: () => void;
    compact?: boolean;
  } = $props();

  const iconMap: Record<string, string> = {
    library: "library",
    history: "clock",
    search: "search",
    sources: "package",
    package: "package",
    error: "alert",
    play: "playCircle",
    filter: "filter",
    database: "database",
    sparkles: "sparkles",
  };

  const iconName = $derived(iconMap[icon] ?? "sparkles");
</script>

<div class="empty-state {compact ? 'is-compact' : ''}">
  <div class="relative text-center max-w-md">
    <div class="empty-icon">
      <Icon name={iconName} size={26} strokeWidth={1.5} />
    </div>
    <h2 class="text-xl font-semibold" style="font-family: var(--font-display);">{title}</h2>
    <p class="text-sm mt-2 leading-relaxed" style="color: var(--paper-300);">{body}</p>
    {#if (actionLabel && onAction) || (secondaryLabel && onSecondary)}
      <div class="mt-5 flex flex-wrap items-center justify-center gap-1.5">
        {#if actionLabel && onAction}
          <button class="btn-primary px-4 py-2.5 text-sm" onclick={onAction}>
            {actionLabel}
            <Icon name={actionIcon} size={14} />
          </button>
        {/if}
        {#if secondaryLabel && onSecondary}
          <button class="empty-link" onclick={onSecondary}>{secondaryLabel}</button>
        {/if}
      </div>
    {/if}
  </div>
</div>
