<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { View } from "./types";

  let {
    view,
    onNav,
    onOpenPalette,
    onOpenShortcuts,
  }: {
    view: View;
    onNav: (next: View) => void;
    onOpenPalette?: () => void;
    onOpenShortcuts?: () => void;
  } = $props();

  // One coherent stroke set: 24px, 1.5 width, currentColor. No emoji in chrome.
  const items: { id: View; label: string; key: string; icon: string }[] = [
    { id: "browse", label: "Browse", key: "1", icon: "grid" },
    { id: "schedule", label: "Schedule", key: "2", icon: "calendar" },
    { id: "library", label: "Library", key: "3", icon: "library" },
    { id: "history", label: "History", key: "4", icon: "clock" },
    { id: "sources", label: "Sources", key: "5", icon: "package" },
    { id: "settings", label: "Settings", key: "6", icon: "settings" },
  ];
</script>

<nav class="nav-rail" aria-label="Primary">
  <div class="flex flex-col gap-1">
    {#each items as item}
      {@const active = view === item.id}
      <button
        class="rail-item"
        aria-current={active ? "page" : undefined}
        aria-label="{item.label} ({item.key})"
        title="{item.label} [{item.key}]"
        onclick={() => onNav(item.id)}
      >
        {#if active}
          <span class="rail-bar" aria-hidden="true"></span>
        {/if}
        <Icon name={item.icon} size={22} class="rail-icon" />
        <span>{item.label}</span>
        <kbd class="rail-key" aria-hidden="true">{item.key}</kbd>
      </button>
    {/each}
  </div>

  <div class="mt-auto flex flex-col items-center gap-1.5 pt-3" style="border-top: 1px solid var(--line);">
    {#if onOpenPalette}
      <button
        class="rail-utility"
        onclick={onOpenPalette}
        aria-label="Open search palette"
        title="Search [Ctrl K]"
      >
        <Icon name="search" size={17} />
      </button>
    {/if}
    {#if onOpenShortcuts}
      <button
        class="rail-utility"
        onclick={onOpenShortcuts}
        aria-label="Keyboard shortcuts"
        title="Keyboard shortcuts [?]"
      >
        <Icon name="help" size={17} />
      </button>
    {/if}
    <span class="mt-1 font-mono text-[8px] tracking-[0.16em]" style="color: var(--paper-500);">{__APP_VERSION__}</span>
  </div>
</nav>
