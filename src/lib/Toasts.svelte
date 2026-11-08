<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { Toast } from "./types";

  let { toasts, onDismiss }: { toasts: Toast[]; onDismiss: (id: number) => void } = $props();

  function iconFor(tone: Toast["tone"]): string {
    if (tone === "success") return "checkCircle";
    if (tone === "error") return "alert";
    return "info";
  }
</script>

<div class="toast-stack" aria-live="polite" aria-relevant="additions">
  {#each toasts as toast (toast.id)}
    <div
      class="toast toast-in"
      data-tone={toast.tone}
      role={toast.tone === "error" ? "alert" : "status"}
    >
      <span class="toast-icon" aria-hidden="true">
        <Icon name={iconFor(toast.tone)} size={15} />
      </span>
      <div class="flex-1 min-w-0">
        <p class="text-sm leading-snug" style="color: var(--paper-50);">{toast.text}</p>
        {#if toast.action}
          <button
            class="mt-1.5 text-[11px] font-semibold"
            style="color: var(--volt-300);"
            onclick={() => {
              toast.action?.run();
              onDismiss(toast.id);
            }}
          >
            {toast.action.label}
          </button>
        {/if}
      </div>
      <button
        class="w-6 h-6 grid place-items-center rounded-md shrink-0"
        style="color: var(--paper-500);"
        onclick={() => onDismiss(toast.id)}
        aria-label="Dismiss notification"
      >
        <Icon name="close" size={13} />
      </button>
      <span class="toast-timer" aria-hidden="true"></span>
    </div>
  {/each}
</div>
