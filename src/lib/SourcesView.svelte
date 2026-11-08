<script lang="ts">
  import EmptyState from "./EmptyState.svelte";
  import type { Boot } from "./types";

  let { boot }: { boot: Boot | null } = $props();

  const cards = [
    {
      title: "Sandboxed",
      body: "QuickJS runs one runtime per extension. Each call times out, memory is capped, and a bad source cannot take the app down.",
      icon: '<path d="M12 3l7 3v5c0 5-3.5 8-7 10-3.5-2-7-5-7-10V6l7-3z" /><path d="M9.5 12l2 2 3.5-4" />',
    },
    {
      title: "Bridge only",
      body: "Extensions reach the net through net.fetch, parse with html.query, and keep small state in storage. No filesystem, no child process, no env.",
      icon: '<path d="M10 14a5 5 0 0 0 7.1 0l2.8-2.8a5 5 0 0 0-7.1-7.1l-1.6 1.6" /><path d="M14 10a5 5 0 0 0-7.1 0l-2.8 2.8a5 5 0 0 0 7.1 7.1l1.6-1.6" />',
    },
    {
      title: "Signed registry",
      body: "Packages install as signed, sideloaded, or disabled. Widening host permissions always asks again. Revoked keys disable on next sync.",
      icon: '<circle cx="12" cy="12" r="8.5" /><path d="M8.5 12.5l2.5 2.5 4.5-5.5" />',
    },
  ];

  const mem = $derived(
    boot ? `${Math.round(boot.limits.extMemoryBytes / 1024 / 1024)}MB per runtime` : "Loading",
  );
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <div class="flex items-baseline gap-3 mb-1">
    <h1 class="text-[28px] font-bold" style="font-family: var(--font-display);">Sources</h1>
    <span class="text-sm" style="color: var(--paper-500);">extension host lands in P1</span>
  </div>
  <p class="text-sm mb-6 max-w-2xl leading-relaxed" style="color: var(--paper-300);">
    AniList covers the catalogue. Playable streams come from extensions you choose. Each one runs fenced off, with its own logs and kill switch.
  </p>

  <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
    {#each cards as card}
      <div class="p-5 rounded-2xl" style="background: var(--void-900); border: 1px solid var(--line);">
        <svg
          width="24"
          height="24"
          viewBox="0 0 24 24"
          fill="none"
          stroke="var(--volt-400)"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          {@html card.icon}
        </svg>
        <h2 class="text-[15px] font-semibold mt-3">{card.title}</h2>
        <p class="text-sm mt-1.5 leading-relaxed" style="color: var(--paper-300);">{card.body}</p>
      </div>
    {/each}
  </div>

  <div
    class="rounded-2xl px-5 py-4 mb-4 flex flex-col sm:flex-row sm:items-center gap-1"
    style="background: var(--void-900); border: 1px solid var(--line);"
  >
    <span class="text-sm font-semibold sm:w-40 shrink-0" style="color: var(--paper-500);">Host budget</span>
    <span class="text-sm" style="color: var(--paper-50); font-family: var(--font-mono);">
      {boot
        ? `${boot.limits.extTimeoutSecs}s calls, ${boot.limits.extResolveTimeoutSecs}s resolves, ${mem}`
        : "Loading"}
    </span>
  </div>

  <div style="border: 1px solid var(--line); border-radius: var(--radius-lg); overflow: hidden;">
    <EmptyState
      icon="◇"
      title="No sources installed"
      body="The host is not wired up yet. When P1 lands, sources install from a registry URL and appear here with per source logs."
    />
  </div>
</div>
