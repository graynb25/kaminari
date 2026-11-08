<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  let { title, onOpenPalette }: { title: string; onOpenPalette: () => void } = $props();

  const appWindow = getCurrentWindow();
</script>

<header
  class="h-10 flex items-center gap-3 px-3 shrink-0 select-none"
  data-tauri-drag-region
  style="background: rgb(14 14 26 / 0.8); border-bottom: 1px solid var(--line); backdrop-filter: blur(16px);"
>
  <div class="flex items-center gap-2 min-w-0" data-tauri-drag-region>
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--volt-400)" stroke-width="1.5" stroke-linejoin="round" aria-hidden="true">
      <path d="M13 2 4 14h6l-1 8 9-12h-6l1-8z" />
    </svg>
    <span class="font-semibold tracking-wide text-sm whitespace-nowrap" data-tauri-drag-region>{title}</span>
  </div>

  <div class="flex-1 flex justify-center" data-tauri-drag-region>
    <button
      class="hidden sm:flex items-center gap-2 h-7 w-[300px] px-3 text-sm rounded-md"
      style="background: var(--void-800); border: 1px solid var(--line); color: var(--paper-500);"
      onclick={onOpenPalette}
      aria-label="Open search palette"
    >
      <span aria-hidden="true">⌕</span>
      <span class="flex-1 text-left">Search AniList</span>
      <kbd
        class="text-[11px] px-1.5 py-0.5 rounded"
        style="background: var(--void-700); color: var(--paper-300); font-family: var(--font-mono);"
      >Ctrl K</kbd>
    </button>
  </div>

  <div class="flex items-center">
    <button class="window-control" aria-label="Minimize" title="Minimize" onclick={() => appWindow.minimize()}>
      <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
        <path d="M1 5h8" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
    <button class="window-control" aria-label="Maximize" title="Maximize" onclick={() => appWindow.toggleMaximize()}>
      <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
        <rect x="1.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
    <button
      class="window-control window-control-close"
      aria-label="Close"
      title="Close"
      onclick={() => appWindow.close()}
    >
      <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
        <path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
  </div>
</header>
