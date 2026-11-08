<script lang="ts">
  let {
    icon,
    title,
    body,
    actionLabel,
    onAction,
  }: {
    icon: string;
    title: string;
    body: string;
    actionLabel?: string;
    onAction?: () => void;
  } = $props();
</script>

<div class="h-full grid place-items-center p-6">
  <div class="text-center max-w-md">
    <p class="text-5xl mb-4" aria-hidden="true" style="color: var(--paper-500);">{icon}</p>
    <h2 class="text-xl font-semibold" style="font-family: var(--font-display);">{title}</h2>
    <p class="text-sm mt-2 leading-relaxed" style="color: var(--paper-300);">{body}</p>
    {#if actionLabel && onAction}
      <button class="btn-primary mt-5 px-4 py-2 text-sm" onclick={onAction}>
        {actionLabel}
      </button>
    {/if}
  </div>
</div>
