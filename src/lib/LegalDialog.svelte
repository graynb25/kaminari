<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import terms from "../../docs/TERMS.md?raw";
  import privacy from "../../docs/PRIVACY.md?raw";

  let {
    initial = "terms",
    onClose,
  }: {
    initial?: "terms" | "privacy";
    onClose: () => void;
  } = $props();

  const docs = { terms, privacy } as const;
  let active = $state<"terms" | "privacy">(initial);
  let dialog = $state<HTMLDivElement | null>(null);
  let contentEl = $state<HTMLDivElement | null>(null);
  let closeButton = $state<HTMLButtonElement | null>(null);
  let previousFocus: HTMLElement | null = null;

  const html = $derived(renderMarkdown(docs[active]));

  function escapeHtml(value: string): string {
    return value
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  function inline(value: string): string {
    return escapeHtml(value)
      .replace(/`([^`]+)`/g, "<code>$1</code>")
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noreferrer">$1</a>');
  }

  function renderMarkdown(markdown: string): string {
    const lines = markdown.replace(/\r\n/g, "\n").split("\n");
    const out: string[] = [];
    let list: "ul" | "ol" | null = null;
    let paragraph: string[] = [];

    function flushParagraph() {
      if (paragraph.length > 0) {
        out.push(`<p>${inline(paragraph.join(" "))}</p>`);
        paragraph = [];
      }
    }

    function closeList() {
      if (list) {
        out.push(`</${list}>`);
        list = null;
      }
    }

    for (const raw of lines) {
      const line = raw.trim();
      if (!line) {
        flushParagraph();
        closeList();
        continue;
      }
      if (line.startsWith("### ")) {
        flushParagraph();
        closeList();
        out.push(`<h3>${inline(line.slice(4))}</h3>`);
        continue;
      }
      if (line.startsWith("## ")) {
        flushParagraph();
        closeList();
        out.push(`<h2>${inline(line.slice(3))}</h2>`);
        continue;
      }
      if (line.startsWith("# ")) {
        flushParagraph();
        closeList();
        out.push(`<h1>${inline(line.slice(2))}</h1>`);
        continue;
      }
      if (line.startsWith("- ")) {
        flushParagraph();
        if (list !== "ul") {
          closeList();
          out.push("<ul>");
          list = "ul";
        }
        out.push(`<li>${inline(line.slice(2))}</li>`);
        continue;
      }
      const ordered = line.match(/^\d+\.\s+(.*)$/);
      if (ordered) {
        flushParagraph();
        if (list !== "ol") {
          closeList();
          out.push("<ol>");
          list = "ol";
        }
        out.push(`<li>${inline(ordered[1])}</li>`);
        continue;
      }
      flushParagraph();
      closeList();
      paragraph.push(line);
    }

    flushParagraph();
    closeList();
    return out.join("\n");
  }

  function switchDoc(next: "terms" | "privacy") {
    active = next;
    requestAnimationFrame(() => {
      if (contentEl) contentEl.scrollTop = 0;
    });
  }

  function trapTab(event: KeyboardEvent) {
    if (event.key !== "Tab" || !dialog) return;
    const items = [...dialog.querySelectorAll<HTMLElement>("button, [href], input, select, textarea, [tabindex='0']")].filter(
      (el) => el.tabIndex >= 0 && el.offsetParent !== null,
    );
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

  onMount(() => {
    previousFocus = document.activeElement as HTMLElement | null;
    document.body.dataset.overlay = "legal";
    requestAnimationFrame(() => closeButton?.focus());
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      delete document.body.dataset.overlay;
      previousFocus?.focus?.();
    };
  });
</script>

<div
  class="legal-backdrop fixed inset-0 z-[80] flex items-center justify-center p-4 overlay-backdrop"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}
  onwheel={(event) => {
    if (event.target === event.currentTarget) event.preventDefault();
  }}
>
  <div
    bind:this={dialog}
    tabindex={-1}
    class="legal-panel glass-strong w-full max-w-[860px] max-h-[86vh] rounded-[22px] overflow-hidden flex flex-col"
    style="border: 1px solid var(--line-strong); box-shadow: var(--shadow-deep);"
    role="dialog"
    aria-modal="true"
    aria-label="Legal documents"
    onkeydown={trapTab}
  >
    <div
      class="shrink-0 flex flex-wrap items-center gap-3 px-5 py-3.5"
      style="border-bottom: 1px solid var(--line);"
    >
      <span
        class="w-9 h-9 grid place-items-center rounded-xl shrink-0"
        style="background: rgb(108 107 245 / 0.12); border: 1px solid var(--line-volt); color: var(--volt-300);"
      >
        <Icon name="shield" size={17} />
      </span>
      <div class="min-w-0 flex-1">
        <h2 class="text-base font-semibold">Legal</h2>
        <p class="text-[11px]" style="color: var(--text-4);">
          Kaminari ships no content and no sources.
        </p>
      </div>
      <div class="legal-tabs" role="tablist" aria-label="Legal documents">
        <button
          id="legal-tab-terms"
          role="tab"
          aria-controls="legal-panel"
          aria-selected={active === "terms"}
          class:is-active={active === "terms"}
          onclick={() => switchDoc("terms")}
        >
          Terms
        </button>
        <button
          id="legal-tab-privacy"
          role="tab"
          aria-controls="legal-panel"
          aria-selected={active === "privacy"}
          class:is-active={active === "privacy"}
          onclick={() => switchDoc("privacy")}
        >
          Privacy
        </button>
      </div>
      <button
        bind:this={closeButton}
        class="btn-ghost px-2.5 py-1.5 text-xs"
        onclick={onClose}
        aria-label="Close legal documents"
      >
        <Icon name="close" size={14} />
      </button>
    </div>

    <div
      bind:this={contentEl}
      class="flex-1 overflow-y-auto px-5 py-5 md:px-7"
      style="overscroll-behavior: contain;"
      role="tabpanel"
      id="legal-panel"
      aria-labelledby={active === "terms" ? "legal-tab-terms" : "legal-tab-privacy"}
      tabindex="0"
    >
      <div class="legal-draft">
        Draft policy. Contact, jurisdiction and repository placeholders are pending legal review.
      </div>
      <div class="legal-markdown">{@html html}</div>
    </div>
  </div>
</div>

<style>
  .legal-tabs {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: 10px;
    background: var(--tint-1);
    border: 1px solid var(--line);
  }

  .legal-tabs button {
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

  .legal-tabs button:hover {
    color: var(--text-1);
  }

  .legal-tabs button.is-active {
    background: var(--surface-1);
    color: var(--text-1);
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.12);
  }
</style>
