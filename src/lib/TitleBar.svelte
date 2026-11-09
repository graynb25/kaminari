<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import BrandMark from "./BrandMark.svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    online,
    onOpenPalette,
  }: { title: string; online: boolean; onOpenPalette: () => void } = $props();

  const appWindow = getCurrentWindow();
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="flex items-center gap-2.5 min-w-0" data-tauri-drag-region>
    <BrandMark size={24} />
    <div class="flex items-baseline gap-2 min-w-0" data-tauri-drag-region>
      <span class="brand-word truncate">{title || "Kaminari"}</span>
      <span class="brand-tag hidden md:inline">ANIME DESKTOP</span>
    </div>
  </div>

  <div class="flex-1 flex justify-center" data-tauri-drag-region>
    <button
      class="palette-trigger hidden sm:flex"
      onclick={onOpenPalette}
      aria-label="Open search palette"
    >
      <Icon name="search" size={14} />
      <span class="flex-1 text-left truncate">Search titles, library, actions</span>
      <kbd class="kbd">Ctrl K</kbd>
    </button>
  </div>

  <div class="flex items-center gap-2 shrink-0">
    <span class="status-pill hidden lg:inline-flex" title={online ? "Network is available" : "You are offline"}>
      <span class="status-dot {online ? '' : 'is-offline'}" aria-hidden="true"></span>
      {online ? "Online" : "Offline"}
    </span>
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
  </div>
</header>
