<script lang="ts">
  import Icon from "./Icon.svelte";
  import { QUALITY_OPTIONS, type QualityPreference } from "./quality";

  let {
    value,
    onChange,
    label = "Preferred playback quality",
    compact = false,
  }: {
    value: QualityPreference;
    onChange: (value: QualityPreference) => void;
    label?: string;
    compact?: boolean;
  } = $props();

  let open = $state(false);
  let placement = $state<"below" | "above">("below");
  let root = $state<HTMLDivElement | null>(null);
  let trigger = $state<HTMLButtonElement | null>(null);
  let popover = $state<HTMLDivElement | null>(null);

  const current = $derived(
    QUALITY_OPTIONS.find((option) => option.value === value) ?? QUALITY_OPTIONS[0],
  );

  function choose(next: QualityPreference) {
    open = false;
    onChange(next);
    requestAnimationFrame(() => trigger?.focus());
  }

  function closeAndRestore() {
    open = false;
    requestAnimationFrame(() => trigger?.focus());
  }

  function toggle() {
    if (!open && root) {
      const rect = root.getBoundingClientRect();
      placement = window.innerHeight - rect.bottom < 320 ? "above" : "below";
    }
    open = !open;
  }

  function onTriggerKeys(event: KeyboardEvent) {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    open = true;
  }

  function onPopoverKeys(event: KeyboardEvent) {
    const items = Array.from(
      popover?.querySelectorAll<HTMLButtonElement>(".quality-choice") ?? [],
    );
    if (items.length === 0) return;
    const currentIndex = items.indexOf(document.activeElement as HTMLButtonElement);
    let next: number | null = null;
    if (event.key === "ArrowDown") next = (currentIndex + 1 + items.length) % items.length;
    else if (event.key === "ArrowUp") next = (currentIndex - 1 + items.length) % items.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = items.length - 1;
    if (next === null) return;
    event.preventDefault();
    items[next]?.focus();
  }

  function onFocusOut(event: FocusEvent) {
    if (!open) return;
    const next = event.relatedTarget;
    if (next instanceof Node && root?.contains(next)) return;
    open = false;
  }

  $effect(() => {
    if (!open) return;
    const frame = requestAnimationFrame(() => {
      const selected = popover?.querySelector<HTMLButtonElement>('[aria-selected="true"]');
      (selected ?? popover?.querySelector<HTMLButtonElement>(".quality-choice"))?.focus();
    });
    return () => cancelAnimationFrame(frame);
  });

  $effect(() => {
    const onPointer = (event: PointerEvent) => {
      if (root && !root.contains(event.target as Node)) open = false;
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && open) {
        event.preventDefault();
        event.stopImmediatePropagation();
        closeAndRestore();
      }
    };
    document.addEventListener("pointerdown", onPointer);
    window.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("pointerdown", onPointer);
      window.removeEventListener("keydown", onKey, true);
    };
  });
</script>

<div
  bind:this={root}
  class="quality-select"
  class:is-compact={compact}
  onfocusout={onFocusOut}
>
  <button
    bind:this={trigger}
    type="button"
    class="quality-trigger"
    aria-label="{label}: {current.label}"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={open ? "quality-listbox" : undefined}
    onclick={toggle}
    onkeydown={onTriggerKeys}
    title={label}
  >
    <span class="quality-value">{current.label}</span>
    <span class="quality-chevron" aria-hidden="true">
      <Icon name="chevronDown" size={12} />
    </span>
  </button>

  {#if open}
    <div
      bind:this={popover}
      class="quality-popover"
      class:is-above={placement === "above"}
      id="quality-listbox"
      role="listbox"
      aria-label={label}
      tabindex={-1}
      onkeydown={onPopoverKeys}
    >
      {#each QUALITY_OPTIONS as option (option.value)}
        {@const active = option.value === value}
        <button
          type="button"
          class="quality-choice"
          class:is-active={active}
          role="option"
          aria-selected={active}
          tabindex={-1}
          onclick={() => choose(option.value)}
        >
          <span>{option.label}</span>
          {#if active}
            <span class="quality-check" aria-hidden="true">
              <Icon name="check" size={13} />
            </span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .quality-select {
    position: relative;
    display: inline-block;
    flex-shrink: 0;
  }

  .quality-trigger {
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-width: 132px;
    height: 32px;
    padding: 0 10px;
    border-radius: 9px;
    border: 1px solid var(--line);
    background: var(--input-bg);
    color: var(--text-1);
    font-size: 12px;
    text-align: left;
    transition: border-color var(--dur-fast) var(--ease-out), background var(--dur-fast) var(--ease-out);
  }

  .quality-trigger:hover,
  .quality-trigger[aria-expanded="true"] {
    border-color: var(--line-volt);
    background: var(--control-bg);
  }

  .quality-select.is-compact .quality-trigger {
    min-width: 104px;
    height: 30px;
    font-size: 11px;
  }

  .quality-value {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .quality-chevron {
    display: grid;
    place-items: center;
    color: var(--paper-450);
    flex-shrink: 0;
  }

  .quality-popover {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    z-index: 70;
    width: 210px;
    max-height: min(340px, 55vh);
    overflow-y: auto;
    padding: 6px;
    border-radius: 13px;
    border: 1px solid var(--line-strong);
    background: var(--void-900);
    box-shadow: var(--shadow-pop);
  }

  .quality-popover.is-above {
    top: auto;
    bottom: calc(100% + 6px);
  }

  .quality-choice {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    width: 100%;
    padding: 8px 9px;
    border-radius: 8px;
    color: var(--paper-200);
    font-size: 12px;
    text-align: left;
    transition: background var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
  }

  .quality-choice:hover {
    background: rgb(255 255 255 / 0.06);
  }

  .quality-choice.is-active {
    background: var(--accent-wash);
    color: var(--paper-50);
  }

  .quality-check {
    display: grid;
    place-items: center;
    color: var(--volt-300);
    flex-shrink: 0;
  }
</style>
