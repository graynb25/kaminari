<script lang="ts">
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let {
    onClose,
    onSources,
    onBrowse,
  }: {
    onClose: () => void;
    onSources: () => void;
    onBrowse: () => void;
  } = $props();

  const steps = [
    {
      icon: "package",
      title: "Add a source",
      body: "Kaminari ships no anime source. Open Sources, paste a raw Mangayomi JavaScript URL, inspect it, and install it only if you trust it. Aniyomi APKs do not run here.",
    },
    {
      icon: "search",
      title: "Find a title",
      body: "Browse AniList without an account. Open a title and choose Watch to search your installed sources for the closest match.",
    },
    {
      icon: "play",
      title: "Watch and keep",
      body: "Pick a stream, set preferred quality, and let the next episode autoplay. Downloads are for content you have the right to keep.",
    },
  ];
</script>

<Modal
  size="md"
  maxHeight="88vh"
  overlayName="getting-started"
  label="Getting started with Kaminari"
  onClose={onClose}
  headerClass="flex items-center gap-3 px-5 py-4"
  bodyClass="px-5 py-5"
>
  {#snippet header()}
    <span
      class="w-9 h-9 grid place-items-center rounded-xl shrink-0"
      style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);"
    >
      <Icon name="sparkles" size={17} />
    </span>
    <div class="min-w-0 flex-1">
      <h2 class="text-base font-semibold">Getting started</h2>
      <p class="text-[11px]" style="color: var(--text-4);">A player, not a catalog.</p>
    </div>
    <button class="btn-ghost px-3 py-1.5 text-xs" onclick={onClose}>
      Close
      <kbd class="kbd">Esc</kbd>
    </button>
  {/snippet}

  <p class="text-sm leading-relaxed" style="color: var(--paper-300);">
    Kaminari is a Windows anime player. AniList supplies discovery and metadata. A source you
    install supplies the streams.
  </p>

  <ol class="mt-5 space-y-4">
    {#each steps as step, index}
      <li class="flex items-start gap-3">
        <span
          class="w-8 h-8 grid place-items-center rounded-lg shrink-0 text-[11px] font-semibold"
          style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-200); font-family: var(--font-mono);"
          aria-hidden="true"
        >
          {index + 1}
        </span>
        <div class="min-w-0">
          <p class="text-sm font-semibold flex items-center gap-2">
            <Icon name={step.icon} size={14} />
            {step.title}
          </p>
          <p class="text-xs mt-1 leading-relaxed" style="color: var(--paper-450);">{step.body}</p>
        </div>
      </li>
    {/each}
  </ol>

  <div
    class="mt-5 rounded-xl p-3 flex items-start gap-2.5 text-xs leading-relaxed"
    style="background: var(--warning-wash); border: 1px solid var(--warning-line); color: var(--paper-300);"
    role="note"
  >
    <Icon name="shield" size={15} class="shrink-0 mt-0.5" />
    <span>Kaminari does not review or endorse third party sources. Install only code you trust.</span>
  </div>

  <div class="flex flex-wrap gap-2 mt-5">
    <button class="btn-primary px-4 py-2 text-sm" onclick={onSources} data-autofocus>
      <Icon name="package" size={14} />
      Add a source
    </button>
    <button class="btn-ghost px-4 py-2 text-sm" onclick={onBrowse}>
      <Icon name="grid" size={14} />
      Browse AniList
    </button>
    <button class="btn-ghost px-4 py-2 text-sm ml-auto" onclick={onClose}>Skip for now</button>
  </div>
</Modal>
