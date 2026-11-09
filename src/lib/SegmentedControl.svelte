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

  let tabEls = $state<Array<HTMLButtonElement | null>>([]);

  function focusTab(index: number) {
    tabEls[index]?.focus();
  }

  function onKeys(event: KeyboardEvent, index: number) {
    let next: number | null = null;
    if (event.key === "ArrowRight") next = (index + 1) % options.length;
    else if (event.key === "ArrowLeft") next = (index - 1 + options.length) % options.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = options.length - 1;
    else return;
    event.preventDefault();
    const option = options[next];
    if (!option) return;
    if (option.id !== value) onChange(option.id);
    focusTab(next);
  }
</script>

<div class="segmented" role="tablist" aria-label={ariaLabel}>
  {#each options as option, index (option.id)}
    <button
      bind:this={tabEls[index]}
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
