<script lang="ts">
  import { untrack } from "svelte";
  import type { Snippet } from "svelte";
  import {
    acquireOverlay,
    focusableElements,
    isolateBackground,
    isTopLayer,
    popLayer,
    pushLayer,
    releaseOverlay,
    trapTabKey,
  } from "./a11y";

  type Size = "sm" | "md" | "lg" | "xl";

  let {
    open = true,
    label,
    labelledBy,
    size = "md",
    placement = "center",
    maxHeight = "86vh",
    bodyMaxHeight = null,
    scrollBody = true,
    overlayName = "modal",
    zIndex = 80,
    onClose,
    header,
    footer,
    children,
    headerClass = "",
    bodyClass = "",
    footerClass = "",
    panel = $bindable<HTMLDivElement | null>(null),
    body = $bindable<HTMLDivElement | null>(null),
  }: {
    open?: boolean;
    label: string;
    labelledBy?: string;
    size?: Size;
    placement?: "center" | "top";
    maxHeight?: string | null;
    bodyMaxHeight?: string | null;
    scrollBody?: boolean;
    overlayName?: string;
    zIndex?: number;
    onClose: () => void;
    header?: Snippet;
    footer?: Snippet;
    children?: Snippet;
    headerClass?: string;
    bodyClass?: string;
    footerClass?: string;
    panel?: HTMLDivElement | null;
    body?: HTMLDivElement | null;
  } = $props();

  const widths: Record<Size, number> = { sm: 680, md: 720, lg: 860, xl: 980 };

  let overlayEl = $state<HTMLDivElement | null>(null);

  function close() {
    onClose();
  }

  $effect(() => {
    if (!open) return;
    const root = untrack(() => overlayEl);
    const panelEl = untrack(() => panel);
    if (!root || !panelEl) return;

    const layer = {};
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    acquireOverlay(overlayName);
    pushLayer(layer);
    // Focus the panel first so the dialog is announced before the background
    // becomes inert. The autofocus target follows on the next frame.
    panelEl.focus({ preventScroll: true });
    const restoreBackground = isolateBackground(root);
    const frame = requestAnimationFrame(() => {
      if (!isTopLayer(layer) || !panelEl.isConnected) return;
      const target =
        panelEl.querySelector<HTMLElement>("[data-autofocus]") ??
        focusableElements(panelEl)[0] ??
        panelEl;
      target.focus({ preventScroll: true });
    });

    const onKey = (event: KeyboardEvent) => {
      if (!isTopLayer(layer)) return;
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopImmediatePropagation();
        close();
        return;
      }
      trapTabKey(event, panelEl);
    };
    const onFocusIn = (event: FocusEvent) => {
      if (!isTopLayer(layer) || !panelEl.isConnected) return;
      const target = event.target;
      if (target instanceof Node && panelEl.contains(target)) return;
      panelEl.focus({ preventScroll: true });
    };

    document.addEventListener("keydown", onKey, true);
    document.addEventListener("focusin", onFocusIn, true);
    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener("keydown", onKey, true);
      document.removeEventListener("focusin", onFocusIn, true);
      restoreBackground();
      popLayer(layer);
      releaseOverlay();
      if (opener?.isConnected) opener.focus({ preventScroll: true });
    };
  });

  const panelStyle = $derived(
    `max-width: ${widths[size]}px;${maxHeight ? ` max-height: ${maxHeight};` : ""}`,
  );
  const bodyStyle = $derived(
    `overscroll-behavior: contain;${bodyMaxHeight ? ` max-height: ${bodyMaxHeight};` : ""}`,
  );
</script>

{#if open}
  <div
    bind:this={overlayEl}
    class="fixed inset-0 flex justify-center overlay-backdrop overlay-in {placement === 'top'
      ? 'items-start px-4 pt-[12vh]'
      : 'items-center p-4'}"
    style="z-index: {zIndex};"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) close();
    }}
    onwheel={(event) => {
      if (event.target === event.currentTarget) event.preventDefault();
    }}
  >
    <div
      bind:this={panel}
      tabindex={-1}
      class="modal-in glass-strong w-full overflow-hidden rounded-[22px] flex flex-col"
      style={panelStyle}
      role="dialog"
      aria-modal="true"
      aria-labelledby={labelledBy}
      aria-label={labelledBy ? undefined : label}
    >
      {#if header}
        <div class="modal-head {headerClass}">{@render header()}</div>
      {/if}
      <div
        bind:this={body}
        class="modal-body {scrollBody ? 'overflow-y-auto' : 'overflow-hidden'} {bodyClass}"
        style={bodyStyle}
      >
        {@render children?.()}
      </div>
      {#if footer}
        <div class="modal-foot {footerClass}">{@render footer()}</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .modal-head {
    position: sticky;
    top: 0;
    z-index: 10;
    flex-shrink: 0;
    background: var(--glass-strong);
    border-bottom: 1px solid var(--line);
  }

  .modal-body {
    flex: 1 1 auto;
    min-height: 0;
  }

  .modal-foot {
    flex-shrink: 0;
    color: var(--text-4);
    border-top: 1px solid var(--line);
  }
</style>
