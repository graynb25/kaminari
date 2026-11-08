<script lang="ts">
  import type { View } from "./types";

  let { view, onNav }: { view: View; onNav: (next: View) => void } = $props();

  // One coherent stroke set: 24px, 1.5 width, currentColor. No emoji in chrome.
  const items: { id: View; label: string; key: string; path: string }[] = [
    {
      id: "browse",
      label: "Browse",
      key: "1",
      path: '<rect x="3" y="3" width="7" height="7" rx="1.5" /><rect x="14" y="3" width="7" height="7" rx="1.5" /><rect x="3" y="14" width="7" height="7" rx="1.5" /><rect x="14" y="14" width="7" height="7" rx="1.5" />',
    },
    {
      id: "library",
      label: "Library",
      key: "2",
      path: '<path d="M6 3h12v18l-6-4.5L6 21V3z" />',
    },
    {
      id: "sources",
      label: "Sources",
      key: "3",
      path: '<path d="M12 2.5 20 7v10l-8 4.5L4 17V7l8-4.5z" /><path d="M12 12 4.2 7.2M12 12l7.8-4.8M12 12v9.5" />',
    },
    {
      id: "settings",
      label: "Settings",
      key: "4",
      path: '<path d="M4 7h10M18 7h2M4 17h4M12 17h8" /><circle cx="16" cy="7" r="2.2" /><circle cx="10" cy="17" r="2.2" />',
    },
  ];
</script>

<nav
  aria-label="Primary"
  class="w-[72px] shrink-0 flex flex-col items-stretch gap-1 py-3 px-2"
  style="background: var(--void-900); border-right: 1px solid var(--line);"
>
  {#each items as item}
    {@const active = view === item.id}
    <button
      class="relative flex flex-col items-center gap-1 py-2.5 rounded-xl text-[11px] font-semibold"
      style="color: {active ? 'var(--paper-50)' : 'var(--paper-500)'}; background: {active ? 'var(--void-800)' : 'transparent'};"
      aria-current={active ? "page" : undefined}
      aria-label="{item.label} ({item.key})"
      title="{item.label} [{item.key}]"
      onclick={() => onNav(item.id)}
    >
      {#if active}
        <span
          class="absolute left-0 top-2.5 bottom-2.5 w-[3px] rounded-full"
          style="background: var(--volt-400);"
          aria-hidden="true"
        ></span>
      {/if}
      <svg
        width="24"
        height="24"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        {@html item.path}
      </svg>
      <span>{item.label}</span>
      <kbd class="text-[10px]" style="color: var(--paper-500); font-family: var(--font-mono);" aria-hidden="true">
        {item.key}
      </kbd>
    </button>
  {/each}
</nav>
