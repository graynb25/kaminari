<script module lang="ts">
  type VisibilityCallback = (visible: boolean) => void;

  let sharedObserver: IntersectionObserver | null = null;
  const visibilityCallbacks = new Map<Element, VisibilityCallback>();

  function getSharedObserver(): IntersectionObserver | null {
    if (typeof IntersectionObserver === "undefined") return null;
    if (!sharedObserver) {
      sharedObserver = new IntersectionObserver(
        (entries) => {
          for (const entry of entries) {
            visibilityCallbacks.get(entry.target)?.(entry.isIntersecting);
          }
        },
        { rootMargin: "200px", threshold: 0 },
      );
    }
    return sharedObserver;
  }

  // One observer for every Artwork on the page. The cleanup unobserves,
  // so a 500 poster grid never holds hundreds of dead entries.
  function watchVisibility(el: Element, onChange: VisibilityCallback): () => void {
    const observer = getSharedObserver();
    if (!observer) {
      onChange(true);
      return () => {};
    }
    visibilityCallbacks.set(el, onChange);
    observer.observe(el);
    return () => {
      visibilityCallbacks.delete(el);
      observer.unobserve(el);
    };
  }
</script>

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
  let visible = $state(false);
  let wrapEl: HTMLSpanElement | null = $state(null);
  let imgEl: HTMLImageElement | null = $state(null);

  const status = $derived(artworkStatus(src, loaded, failed));

  // A new URL gets its own shimmer. Reset before the cached check runs.
  $effect(() => {
    src;
    loaded = false;
    failed = false;
  });

  // Gate the shimmer to the viewport. The frame stays reserved, so an
  // offscreen image costs nothing until it scrolls near.
  $effect(() => {
    const el = wrapEl;
    if (!el || loaded || failed || !src) return;
    return watchVisibility(el, (next) => {
      visible = next;
    });
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

<span
  bind:this={wrapEl}
  class="artwork {fill ? 'is-fill' : ''} {status === 'ready' ? 'is-loaded' : ''}"
  data-status={status}
>
  {#if src && !failed}
    {#if !loaded && visible}
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
