<script lang="ts">
  import type { Toast } from "./types";

  let { toasts, onDismiss }: { toasts: Toast[]; onDismiss: (id: number) => void } = $props();
</script>

<div class="fixed bottom-4 right-4 z-50 flex flex-col gap-2 w-[320px]" aria-live="polite">
  {#each toasts as toast (toast.id)}
    <div
      class="toast-in glass px-3.5 py-3 rounded-xl text-sm flex items-start gap-2.5"
      style="border: 1px solid var(--line); box-shadow: var(--shadow-pop);"
      role={toast.tone === "error" ? "alert" : "status"}
    >
      <span aria-hidden="true">
        {#if toast.tone === "success"}✓{:else if toast.tone === "error"}!{:else}i{/if}
      </span>
      <p class="flex-1 leading-snug" style="color: var(--paper-50);">{toast.text}</p>
      <button
        class="text-xs shrink-0 px-1.5 py-0.5 rounded"
        style="color: var(--paper-500);"
        onclick={() => onDismiss(toast.id)}
        aria-label="Dismiss notification"
      >
        ✕
      </button>
    </div>
  {/each}
</div>
