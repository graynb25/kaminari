<script lang="ts">
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import SegmentedControl from "./SegmentedControl.svelte";
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
  let active = $state<"terms" | "privacy">("terms");
  let contentEl = $state<HTMLDivElement | null>(null);

  // Follow the prop if the dialog is reused, but keep switchDoc() free to
  // change the tab without an effect overwriting it.
  $effect.pre(() => {
    active = initial;
  });

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
</script>

<Modal
  size="lg"
  maxHeight="86vh"
  overlayName="legal"
  label="Legal documents"
  labelledBy="legal-dialog-title"
  onClose={onClose}
  bind:body={contentEl}
  bodyClass="px-5 py-5 md:px-7"
  headerClass="flex flex-wrap items-center gap-3 px-5 py-3.5"
>
  {#snippet header()}
    <span
      class="w-9 h-9 grid place-items-center rounded-xl shrink-0"
      style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);"
    >
      <Icon name="shield" size={17} />
    </span>
    <div class="min-w-0 flex-1">
      <h2 id="legal-dialog-title" class="text-base font-semibold">Legal</h2>
      <p class="text-[11px]" style="color: var(--text-4);">
        Kaminari ships no content and no sources.
      </p>
    </div>
    <SegmentedControl
      options={[
        { id: "terms", label: "Terms" },
        { id: "privacy", label: "Privacy" },
      ]}
      value={active}
      onChange={(id) => switchDoc(id as "terms" | "privacy")}
      ariaLabel="Legal documents"
      idPrefix="legal"
    />
    <button
      class="btn-ghost px-2.5 py-1.5 text-xs"
      onclick={onClose}
      aria-label="Close legal documents"
      data-autofocus
    >
      <Icon name="close" size={14} />
    </button>
  {/snippet}

  <div
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
</Modal>
