<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    eyebrow,
    title,
    subtitle,
    children,
  }: {
    eyebrow: string;
    title: string;
    subtitle?: string;
    children?: Snippet;
  } = $props();
</script>

<header class="page-head">
  <span class="page-head-glow" aria-hidden="true"></span>
  <div class="page-head-text">
    <p class="eyebrow">{eyebrow}</p>
    <h1 class="page-title">{title}</h1>
    {#if subtitle}
      <p class="page-sub">{subtitle}</p>
    {/if}
  </div>
  <div class="page-head-actions">{@render children?.()}</div>
</header>
