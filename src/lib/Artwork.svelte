<script lang="ts">
  import type { Snippet } from "svelte";
  import { artworkStatus, isArtworkBroken, isArtworkComplete } from "./artwork";

  let {
    src,
    alt = "",
    loading = "lazy",
    imgClass = "",
    fill = false,
    fallback,
  }: {
    src?: string | null;
    alt?: string;
    loading?: "lazy" | "eager";
    imgClass?: string;
    fill?: boolean;
    fallback?: Snippet;
  } = $props();

  let loaded = $state(false);
  let failed = $state(false);
  let imgEl: HTMLImageElement | null = $state(null);

  const status = $derived(artworkStatus(src, loaded, failed));

  // A new URL gets its own shimmer. Reset before the cached check runs.
  $effect(() => {
    src;
    loaded = false;
    failed = false;
  });

  // Cached images are already complete on mount. Show them at once
  // instead of flashing the shimmer, and catch cached broken files.
  $effect(() => {
    if (!src || failed) return;
    const el = imgEl;
    if (!el) return;
    if (isArtworkComplete(el)) loaded = true;
    else if (isArtworkBroken(el)) failed = true;
  });

  function onLoad() {
    loaded = true;
  }

  function onError() {
    failed = true;
  }
</script>

<span class="artwork {fill ? 'is-fill' : ''} {status === 'ready' ? 'is-loaded' : ''}" data-status={status}>
  {#if src && !failed}
    {#if !loaded}
      <span class="artwork-shimmer shimmer" aria-hidden="true"></span>
    {/if}
    <img
      bind:this={imgEl}
      src={src}
      {alt}
      {loading}
      decoding="async"
      class="artwork-img {imgClass}"
      onload={onLoad}
      onerror={onError}
    />
  {:else if fallback}
    {@render fallback()}
  {/if}
</span>
