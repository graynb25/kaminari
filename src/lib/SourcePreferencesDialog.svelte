<script lang="ts">
  import { onMount } from "svelte";
  import Callout from "./Callout.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import { jsExtensionPreferences, jsExtensionSetPreference } from "./api";
  import type {
    SourcePreferenceItem,
    SourcePreferenceList,
    SourcePreferenceSwitch,
  } from "./types";

  let {
    extension,
    onClose,
    notify,
  }: {
    extension: { id: string; name: string };
    onClose: () => void;
    notify: (text: string, tone?: "info" | "success" | "error") => void;
  } = $props();

  let items = $state<SourcePreferenceItem[]>([]);
  let values = $state<Record<string, string>>({});
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busyKey = $state<string | null>(null);
  let disposed = false;

  onMount(() => {
    void load();
    return () => {
      disposed = true;
    };
  });

  function switchOf(item: SourcePreferenceItem): SourcePreferenceSwitch | null {
    return item.switchPreferenceCompat ?? item.switchPreference ?? null;
  }

  function listOf(item: SourcePreferenceItem): SourcePreferenceList | null {
    return item.listPreference ?? null;
  }

  function switchValue(item: SourcePreferenceItem): boolean {
    const stored = values[item.key];
    if (stored === "true") return true;
    if (stored === "false") return false;
    return switchOf(item)?.value === true;
  }

  function listValue(item: SourcePreferenceItem): string {
    const descriptor = listOf(item);
    const entries = descriptor?.entries ?? [];
    const optionValues = descriptor?.values ?? entries;
    const stored = values[item.key];
    if (stored !== undefined && optionValues.includes(stored)) return stored;
    const index = Math.max(0, Math.min(entries.length - 1, descriptor?.valueIndex ?? 0));
    return optionValues[index] ?? "";
  }

  async function save(key: string, value: string) {
    busyKey = key;
    error = null;
    try {
      await jsExtensionSetPreference(extension.id, key, value);
      values = { ...values, [key]: value };
      notify("Source preference saved. Reopen the show to apply it.", "success");
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busyKey = null;
    }
  }

  async function load() {
    loading = true;
    error = null;
    try {
      const result = await jsExtensionPreferences(extension.id);
      if (disposed) return;
      items = result.items ?? [];
      values = result.values ?? {};
    } catch (e) {
      if (!disposed) error = e instanceof Error ? e.message : String(e);
    } finally {
      if (!disposed) loading = false;
    }
  }
</script>

<Modal
  size="md"
  maxHeight="86vh"
  overlayName="source-preferences"
  label="{extension.name} settings"
  onClose={onClose}
  headerClass="flex items-center gap-3 px-5 py-4"
  bodyClass="px-5 py-4"
>
  {#snippet header()}
    <span
      class="w-9 h-9 grid place-items-center rounded-xl shrink-0"
      style="background: var(--accent-wash); border: 1px solid var(--line-volt); color: var(--volt-300);"
    >
      <Icon name="settings" size={17} />
    </span>
    <div class="min-w-0 flex-1">
      <h2 class="text-base font-semibold">Source settings</h2>
      <p class="text-[11px] truncate" style="color: var(--text-4);">{extension.name}</p>
    </div>
    <button class="btn-ghost px-3 py-1.5 text-xs" onclick={onClose}>Close</button>
  {/snippet}

  <p class="text-xs mb-4 leading-relaxed" style="color: var(--paper-450);">
    These controls are provided by the source. Changes apply the next time it loads a show.
    Enabling extra requests, such as episode thumbnails, can make a detail page slower.
  </p>

  {#if error}
    <Callout tone="danger" class="mb-4">{error}</Callout>
  {/if}

  {#if loading}
    <ProgressBar label="Reading source settings" indeterminate />
  {:else if items.length === 0}
    <Callout tone="info">This source does not expose any settings.</Callout>
  {:else}
    <div class="space-y-2">
      {#each items as item (item.key)}
        {@const toggle = switchOf(item)}
        {@const list = listOf(item)}
        <div
          class="rounded-xl p-3"
          style="background: var(--well-bg); border: 1px solid var(--line);"
        >
          {#if toggle}
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div class="min-w-0">
                <p class="text-sm font-semibold">{toggle.title ?? item.key}</p>
                {#if toggle.summary}
                  <p class="text-[11px] mt-0.5 leading-relaxed" style="color: var(--paper-450);">
                    {toggle.summary}
                  </p>
                {/if}
              </div>
              <button
                class="chip text-[10px] px-2 py-1 shrink-0"
                aria-pressed={switchValue(item)}
                disabled={busyKey === item.key}
                onclick={() => void save(item.key, switchValue(item) ? "false" : "true")}
              >
                <span
                  class="status-dot {switchValue(item) ? '' : 'is-offline'}"
                  style="width: 4px; height: 4px;"
                  aria-hidden="true"
                ></span>
                {busyKey === item.key ? "Saving" : switchValue(item) ? "On" : "Off"}
              </button>
            </div>
          {:else if list}
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div class="min-w-0">
                <p class="text-sm font-semibold">{list.title ?? item.key}</p>
                {#if list.summary}
                  <p class="text-[11px] mt-0.5 leading-relaxed" style="color: var(--paper-450);">
                    {list.summary}
                  </p>
                {/if}
              </div>
              <select
                class="h-8 rounded-lg px-2 text-xs outline-none shrink-0"
                style="background: var(--input-bg); border: 1px solid var(--line); color: var(--text-1);"
                value={listValue(item)}
                disabled={busyKey === item.key}
                onchange={(event) => void save(item.key, (event.currentTarget as HTMLSelectElement).value)}
                aria-label={list.title ?? item.key}
              >
                {#each list.entries ?? [] as entry, index}
                  <option value={(list.values ?? list.entries ?? [])[index] ?? entry}>
                    {entry}
                  </option>
                {/each}
              </select>
            </div>
          {:else}
            <p class="text-sm font-semibold">{item.key}</p>
            <p class="text-[11px] mt-0.5" style="color: var(--paper-450);">
              This preference type is not editable in Kaminari yet.
            </p>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</Modal>
