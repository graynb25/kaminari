// Shared keyboard and modality helpers. The overlay lock is counted so nested
// layers (a player over the source browser, a dialog over the app) unwind in
// the order they opened.

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "area[href]",
  "button:not([disabled])",
  "input:not([disabled]):not([type='hidden'])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "iframe",
  "object",
  "embed",
  "summary",
  "audio[controls]",
  "video[controls]",
  "[contenteditable='true']",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

const SKIP_ISOLATION = new Set(["SCRIPT", "STYLE", "LINK", "NOSCRIPT", "TEMPLATE"]);

let overlays: string[] = [];
let previousOverlay: string | undefined;
let layers: object[] = [];

export function acquireOverlay(name: string): void {
  if (overlays.length === 0) previousOverlay = document.body.dataset.overlay;
  overlays.push(name);
  document.body.dataset.overlay = name;
}

export function releaseOverlay(): void {
  if (overlays.length === 0) return;
  overlays.pop();
  const current = overlays[overlays.length - 1];
  if (current) {
    document.body.dataset.overlay = current;
    return;
  }
  if (previousOverlay) document.body.dataset.overlay = previousOverlay;
  else delete document.body.dataset.overlay;
  previousOverlay = undefined;
}

export function pushLayer(layer: object): void {
  layers.push(layer);
}

export function popLayer(layer: object): void {
  const index = layers.lastIndexOf(layer);
  if (index >= 0) layers.splice(index, 1);
}

export function isTopLayer(layer: object): boolean {
  return layers[layers.length - 1] === layer;
}

export function isElementVisible(element: HTMLElement): boolean {
  if (!element.isConnected) return false;
  if (element.getAttribute("aria-hidden") === "true") return false;
  if (element.closest("[inert]")) return false;
  const style = window.getComputedStyle(element);
  return (
    style.display !== "none" &&
    style.visibility !== "hidden" &&
    element.getClientRects().length > 0
  );
}

export function focusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => element.tabIndex >= 0 && isElementVisible(element),
  );
}

export function nextFocusIndex(current: number, length: number, direction: 1 | -1): number {
  if (length <= 0) return -1;
  if (current < 0) return direction > 0 ? 0 : length - 1;
  return (current + direction + length) % length;
}

// Keep Tab inside the top layer. A focused panel itself jumps to the first or
// last control so Shift+Tab never leaves through the back door.
export function trapTabKey(event: KeyboardEvent, container: HTMLElement): void {
  if (event.key !== "Tab") return;
  const items = focusableElements(container);
  if (items.length === 0) {
    event.preventDefault();
    container.focus({ preventScroll: true });
    return;
  }
  const first = items[0];
  const last = items[items.length - 1];
  const active = document.activeElement as HTMLElement | null;
  if (!active || !container.contains(active) || active === container) {
    event.preventDefault();
    (event.shiftKey ? last : first).focus();
    return;
  }
  if (event.shiftKey && active === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && active === last) {
    event.preventDefault();
    first.focus();
  }
}

type HiddenSibling = {
  element: HTMLElement;
  inert: boolean;
  ariaHidden: string | null;
};

// Hide every sibling around the modal's ancestor chain from assistive tech and
// remove it from the tab order. Live regions stay available so async feedback
// still reaches the user while a dialog or the player is open.
export function isolateBackground(root: HTMLElement): () => void {
  const hidden: HiddenSibling[] = [];
  let node: Element | null = root;
  while (node && node !== document.body) {
    const parent: HTMLElement | null = node.parentElement;
    if (!parent) break;
    for (const sibling of Array.from(parent.children)) {
      if (sibling === node || !(sibling instanceof HTMLElement)) continue;
      if (SKIP_ISOLATION.has(sibling.tagName)) continue;
      if (sibling.hasAttribute("aria-live") || sibling.classList.contains("toast-stack")) continue;
      hidden.push({
        element: sibling,
        inert: sibling.inert,
        ariaHidden: sibling.getAttribute("aria-hidden"),
      });
      sibling.inert = true;
      sibling.setAttribute("aria-hidden", "true");
    }
    node = parent;
  }
  return () => {
    for (let index = hidden.length - 1; index >= 0; index -= 1) {
      const item = hidden[index];
      item.element.inert = item.inert;
      if (item.ariaHidden === null) item.element.removeAttribute("aria-hidden");
      else item.element.setAttribute("aria-hidden", item.ariaHidden);
    }
  };
}
