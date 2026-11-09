<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    size = 24,
  }: {
    size?: 24 | 28 | 48 | 64;
  } = $props();

  const iconSize = $derived(size === 24 ? 14 : size === 28 ? 16 : size === 48 ? 26 : 34);
  const radius = $derived(size === 24 ? 8 : size === 28 ? 9 : size === 48 ? 14 : 18);
</script>

<span
  class="brandmark"
  style="width: {size}px; height: {size}px; border-radius: {radius}px;"
  aria-hidden="true"
>
  <Icon name="bolt" size={iconSize} strokeWidth={1.8} />
</span>

<style>
  .brandmark {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    background: linear-gradient(180deg, var(--accent-wash-strong), var(--accent-wash));
    border: 1px solid var(--line-volt);
    color: var(--volt-300);
    box-shadow: 0 0 18px var(--accent-shadow);
  }
</style>
