<script lang="ts">
  let {
    options,
    value,
    onChange,
    ariaLabel,
    idPrefix = "seg",
  }: {
    options: { id: string; label: string }[];
    value: string;
    onChange: (id: string) => void;
    ariaLabel: string;
    idPrefix?: string;
  } = $props();

  function onKeys(event: KeyboardEvent, index: number) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const delta = event.key === "ArrowRight" ? 1 : -1;
    const next = options[(index + delta + options.length) % options.length];
    if (next) onChange(next.id);
  }
</script>

<div class="segmented" role="tablist" aria-label={ariaLabel}>
  {#each options as option, index (option.id)}
    <button
      id="{idPrefix}-tab-{option.id}"
      role="tab"
      aria-selected={value === option.id}
      aria-controls="{idPrefix}-panel"
      tabindex={value === option.id ? 0 : -1}
      class:is-active={value === option.id}
      onclick={() => onChange(option.id)}
      onkeydown={(event) => onKeys(event, index)}
    >
      {option.label}
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: 10px;
    background: var(--tint-1);
    border: 1px solid var(--line);
  }

  .segmented button {
    height: 26px;
    padding: 0 10px;
    border-radius: 7px;
    color: var(--text-4);
    font-size: 11px;
    font-weight: 650;
    transition:
      background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .segmented button:hover {
    color: var(--text-1);
  }

  .segmented button.is-active {
    background: var(--surface-1);
    color: var(--text-1);
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.12);
  }
</style>
