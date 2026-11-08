<script lang="ts">
  let { open, onClose }: { open: boolean; onClose: () => void } = $props();

  const groups: { title: string; rows: [string, string][] }[] = [
    {
      title: "Navigate",
      rows: [
        ["Ctrl K or /", "Search palette"],
        ["1 to 5", "Browse, Library, History, Sources, Settings"],
        ["Esc", "Close top layer"],
        ["?", "This dialog"],
      ],
    },
    {
      title: "Cards",
      rows: [
        ["Enter", "Open details"],
        ["P", "Play (opens details with the P1 note)"],
        ["L", "Save or remove"],
      ],
    },
    {
      title: "Hero",
      rows: [
        ["Left and Right", "Move slides"],
      ],
    },
  ];
</script>

{#if open}
  <div
    class="fixed inset-0 z-50 grid place-items-center px-4"
    style="background: rgb(4 4 8 / 0.6);"
    onclick={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}
    role="presentation"
  >
    <div
      class="modal-in glass w-full max-w-[440px] rounded-[20px] p-5"
      style="border: 1px solid var(--line); box-shadow: var(--shadow-pop);"
      role="dialog"
      aria-modal="true"
      aria-label="Keyboard shortcuts"
    >
      <div class="flex items-center gap-3 mb-4">
        <h2 class="text-lg font-semibold flex-1" style="font-family: var(--font-display);">Shortcuts</h2>
        <button class="btn-ghost px-3 py-1.5 text-xs" onclick={onClose}>Close</button>
      </div>
      {#each groups as group}
        <h3 class="eyebrow mt-4 mb-2">{group.title}</h3>
        <dl class="space-y-1.5">
          {#each group.rows as [key, what]}
            <div class="flex items-center gap-3 text-sm">
              <kbd
                class="px-2 py-0.5 rounded-md text-xs shrink-0"
                style="background: var(--void-700); border: 1px solid var(--line); color: var(--paper-50); font-family: var(--font-mono);"
              >
                {key}
              </kbd>
              <dt style="color: var(--paper-300);">{what}</dt>
            </div>
          {/each}
        </dl>
      {/each}
    </div>
  </div>
{/if}
