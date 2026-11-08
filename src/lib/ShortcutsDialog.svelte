<script lang="ts">
  import Icon from "./Icon.svelte";

  let { open, onClose }: { open: boolean; onClose: () => void } = $props();

  const groups: { title: string; icon: string; rows: [string, string][] }[] = [
    {
      title: "Navigate",
      icon: "grid",
      rows: [
        ["Ctrl K or /", "Search palette"],
        ["1 to 5", "Browse, Library, History, Sources, Settings"],
        ["Esc", "Close the top layer"],
        ["?", "This dialog"],
      ],
    },
    {
      title: "Cards",
      icon: "film",
      rows: [
        ["Enter", "Open details"],
        ["P", "Play"],
        ["L", "Save or remove"],
      ],
    },
    {
      title: "Hero",
      icon: "sparkles",
      rows: [["Left and Right", "Move slides"]],
    },
    {
      title: "Search",
      icon: "search",
      rows: [
        ["Up and Down", "Move results"],
        ["Enter", "Open result"],
        ["Shift Enter", "Play result"],
        ["Esc", "Close palette"],
      ],
    },
    {
      title: "Player",
      icon: "playCircle",
      rows: [
        ["Space or K", "Play and pause"],
        ["J and L", "Jump 10 seconds"],
        ["Left and Right", "Jump 10 seconds"],
        ["Shift arrows", "Jump 60 seconds"],
        ["Up and Down", "Volume"],
        ["M", "Mute"],
        ["F", "Fullscreen"],
        ["T", "Fill the frame"],
        ["V", "Cycle subtitles"],
        [", and .", "Step speed"],
        ["0", "Reset speed"],
        ["D", "Diagnostics"],
        ["Esc", "Back"],
      ],
    },
  ];

  let dialog: HTMLDivElement | null = $state(null);
  let closeButton: HTMLButtonElement | null = $state(null);
  let previousFocus: HTMLElement | null = null;

  $effect(() => {
    if (open) {
      previousFocus = document.activeElement as HTMLElement | null;
      requestAnimationFrame(() => closeButton?.focus());
    } else {
      previousFocus?.focus?.();
    }
  });

  $effect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      } else if (event.key === "Tab" && dialog) {
        const items = [...dialog.querySelectorAll("button, [href], input, select, textarea")].filter(
          (el) => (el as HTMLElement).tabIndex >= 0,
        ) as HTMLElement[];
        if (items.length === 0) return;
        const first = items[0];
        const last = items[items.length - 1];
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

{#if open}
  <div
    class="fixed inset-0 z-[60] grid place-items-center px-4 overlay-backdrop overlay-in"
    onclick={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}
    role="presentation"
  >
    <div
      bind:this={dialog}
      tabindex={-1}
      class="modal-in glass-strong w-full max-w-[720px] max-h-[86vh] overflow-y-auto rounded-[22px]"
      role="dialog"
      aria-modal="true"
      aria-label="Keyboard shortcuts"
    >
      <div class="flex items-center gap-3 px-5 py-4 sticky top-0 z-10" style="border-bottom: 1px solid var(--line); background: var(--glass-strong); backdrop-filter: blur(20px);">
        <span class="w-9 h-9 grid place-items-center rounded-xl" style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);">
          <Icon name="keyboard" size={18} />
        </span>
        <div class="min-w-0 flex-1">
          <h2 class="text-lg font-semibold" style="font-family: var(--font-display);">Keyboard shortcuts</h2>
          <p class="text-[11px]" style="color: var(--text-4);">Everything reachable without a mouse.</p>
        </div>
        <button bind:this={closeButton} class="btn-ghost px-3 py-1.5 text-xs" onclick={onClose}>
          Close
          <kbd class="kbd">Esc</kbd>
        </button>
      </div>

      <div class="p-5 grid grid-cols-1 md:grid-cols-2 gap-x-8 gap-y-6">
        {#each groups as group}
          <section class={group.title === "Player" ? "md:col-span-2" : ""} aria-label={group.title}>
            <div class="flex items-center gap-2 mb-2.5">
              <span style="color: var(--volt-300);"><Icon name={group.icon} size={14} /></span>
              <h3 class="text-[10px] font-semibold uppercase tracking-[0.18em]" style="color: var(--text-4);">{group.title}</h3>
            </div>
            <dl class={group.title === "Player" ? "grid grid-cols-1 sm:grid-cols-2 gap-x-8 gap-y-1.5" : "space-y-1.5"}>
              {#each group.rows as [key, what]}
                <div class="flex items-center gap-3 text-sm">
                  <kbd
                    class="px-2 py-1 rounded-md text-[11px] shrink-0 min-w-[54px] text-center"
                    style="background: var(--tint-1); border: 1px solid var(--line-strong); color: var(--text-1); font-family: var(--font-mono);"
                  >
                    {key}
                  </kbd>
                  <dt style="color: var(--text-2);">{what}</dt>
                </div>
              {/each}
            </dl>
          </section>
        {/each}
      </div>

      <div class="px-5 py-3 flex items-center gap-2 text-[10px]" style="border-top: 1px solid var(--line); color: var(--text-4);">
        <Icon name="info" size={12} />
        Player keys work while the player is open. Press ? anywhere outside a text field.
      </div>
    </div>
  </div>
{/if}
