<script module lang="ts">
  // Modal instances stack, so the lock is a shared count. The first one keeps
  // the value that was already on the body and the last one puts it back.
  let overlayLocks = 0;
  let previousOverlay: string | undefined;

  function acquireOverlay(name: string) {
    if (overlayLocks === 0) previousOverlay = document.body.dataset.overlay;
    overlayLocks += 1;
    document.body.dataset.overlay = name;
  }

  function releaseOverlay() {
    overlayLocks = Math.max(0, overlayLocks - 1);
    if (overlayLocks > 0) return;
    if (previousOverlay) document.body.dataset.overlay = previousOverlay;
    else delete document.body.dataset.overlay;
    previousOverlay = undefined;
  }
</script>

<script lang="ts">
  import type { Snippet } from "svelte";

  type Size = "sm" | "md" | "lg" | "xl";

  let {
    open = true,
    label,
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

  function close() {
    onClose();
  }

  function focusable(): HTMLElement[] {
    if (!panel) return [];
    return [
      ...panel.querySelectorAll<HTMLElement>(
        "button, [href], input, select, textarea, [tabindex='0']",
      ),
    ].filter((el) => el.tabIndex >= 0 && el.offsetParent !== null);
  }

  function trapTab(event: KeyboardEvent) {
    if (event.key !== "Tab" || !panel) return;
    const items = focusable();
    if (items.length === 0) {
      event.preventDefault();
      panel.focus();
      return;
    }
    const first = items[0];
    const last = items[items.length - 1];
    const active = document.activeElement;
    if (!panel.contains(active)) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
    } else if (event.shiftKey && active === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first.focus();
    }
  }

  let previousFocus: HTMLElement | null = null;

  $effect(() => {
    if (!open) return;
    previousFocus = document.activeElement as HTMLElement | null;
    acquireOverlay(overlayName);
    const frame = requestAnimationFrame(() => {
      const target =
        panel?.querySelector<HTMLElement>("[data-autofocus]") ?? focusable()[0] ?? panel;
      target?.focus();
    });
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      close();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("keydown", onKey);
      releaseOverlay();
      previousFocus?.focus?.();
      previousFocus = null;
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
      aria-label={label}
      onkeydown={trapTab}
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
