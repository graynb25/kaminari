<script lang="ts">
  import Icon from "./Icon.svelte";
  import { streamLabel } from "./quality";
  import type { JsStream } from "./types";

  let {
    options,
    activeUrl,
    onSelect,
    onClose,
  }: {
    options: JsStream[];
    activeUrl: string;
    onSelect: (stream: JsStream) => void;
    onClose: () => void;
  } = $props();

  let root = $state<HTMLDivElement | null>(null);

  $effect(() => {
    const onPointer = (event: PointerEvent) => {
      const target = event.target as Element | null;
      if (target?.closest("[data-quality-toggle]")) return;
      if (root && !root.contains(event.target as Node)) onClose();
    };
    document.addEventListener("pointerdown", onPointer);
    return () => document.removeEventListener("pointerdown", onPointer);
  });
</script>

<div
  bind:this={root}
  class="player-quality-menu player-glass"
  role="dialog"
  aria-label="Stream quality"
>
  <div class="flex items-center gap-2.5">
    <span class="quality-icon" aria-hidden="true">
      <Icon name="sliders" size={15} />
    </span>
    <div class="min-w-0 flex-1">
      <p class="eyebrow">Player</p>
      <p class="text-sm font-semibold">Stream quality</p>
    </div>
    <button
      class="quality-close"
      onclick={onClose}
      aria-label="Close stream quality"
      title="Close"
    >
      <Icon name="close" size={13} />
    </button>
  </div>

  <div class="mt-3 space-y-1">
    {#each options as option, index (option.url)}
      {@const active = option.url === activeUrl}
      <button
        class="quality-option"
        class:is-active={active}
        onclick={() => onSelect(option)}
        aria-pressed={active}
        title={streamLabel(option)}
      >
        <span class="quality-name">{option.quality?.trim() || `Stream ${index + 1}`}</span>
        <span class="quality-kind">{option.type.toUpperCase()}</span>
        {#if active}
          <span class="quality-check" aria-hidden="true">
            <Icon name="check" size={13} />
          </span>
        {/if}
      </button>
    {/each}
  </div>

  <p class="quality-note">Switches at the current position.</p>
</div>

<style>
  .player-quality-menu {
    position: absolute;
    right: 0;
    bottom: calc(100% + 10px);
    z-index: 30;
    width: 290px;
    max-width: calc(100vw - 24px);
    padding: 13px;
    border-radius: 16px;
    box-shadow: 0 18px 50px rgb(0 0 0 / 0.5);
  }

  .quality-icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    background: var(--accent-wash);
    border: 1px solid var(--line-volt);
    color: var(--volt-300);
    flex-shrink: 0;
  }

  .quality-close {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    color: var(--paper-450);
    flex-shrink: 0;
  }

  .quality-close:hover {
    background: rgb(255 255 255 / 0.08);
    color: var(--paper-100);
  }

  .quality-option {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 8px 9px;
    border-radius: 10px;
    border: 1px solid transparent;
    color: var(--paper-200);
    text-align: left;
    transition: background var(--dur-fast) var(--ease-out), border-color var(--dur-fast) var(--ease-out);
  }

  .quality-option:hover {
    background: rgb(255 255 255 / 0.07);
  }

  .quality-option.is-active {
    background: var(--accent-wash);
    border-color: var(--line-volt);
    color: var(--paper-50);
  }

  .quality-name {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    font-weight: 600;
  }

  .quality-kind {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 9px;
    color: var(--paper-450);
  }

  .quality-check {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    color: var(--volt-300);
  }

  .quality-note {
    margin-top: 9px;
    font-size: 10px;
    color: var(--paper-450);
  }
</style>
