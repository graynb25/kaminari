<script lang="ts">
  import type { Boot } from "./types";

  let { boot }: { boot: Boot | null } = $props();

  const iconProps =
    'width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--volt-400)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"';

  // Static section shape. The two rows that depend on the async boot snapshot
  // resolve through rowValue below, so the template stays reactive.
  const sections = [
    {
      title: "App",
      icon: `<svg ${iconProps}><circle cx="12" cy="12" r="8.5" /><path d="M12 8h.01M12 11.5V16" /></svg>`,
      rows: [
        { label: "Version", value: "0.1.0 scaffold" },
        {
          label: "Stack",
          value: "Tauri 2, Svelte 5 Runes, libmpv, QuickJS host in P1",
        },
      ],
    },
    {
      title: "Data",
      icon: `<svg ${iconProps}><ellipse cx="12" cy="5.5" rx="7.5" ry="2.8" /><path d="M4.5 5.5v13c0 1.6 3.4 2.8 7.5 2.8s7.5-1.2 7.5-2.8v-13" /><path d="M4.5 12c0 1.6 3.4 2.8 7.5 2.8s7.5-1.2 7.5-2.8" /></svg>`,
      rows: [
        { label: "AniList endpoint", value: "" },
        { label: "Library", value: "One local SQLite file, WAL mode, portable backup" },
      ],
    },
    {
      title: "Appearance",
      icon: `<svg ${iconProps}><path d="M20 13.5A8 8 0 1 1 10.5 4 6.5 6.5 0 0 0 20 13.5z" /></svg>`,
      rows: [
        { label: "Theme", value: "Dark is home. Light lands in P2." },
        { label: "Motion", value: "Follows your OS reduce motion setting" },
      ],
    },
    {
      title: "Updates and diagnostics",
      icon: `<svg ${iconProps}><path d="M20 12a8 8 0 1 1-2.4-5.7" /><path d="M20 3v4.5h-4.5" /></svg>`,
      rows: [
        {
          label: "Extension budget",
          value: "",
        },
        { label: "Auto update", value: "One click restart, lands in P3" },
        { label: "Diagnostics export", value: "Redacted bundle, lands in P3" },
      ],
    },
  ];
  function rowValue(label: string, fallback: string): string {
    if (label === "AniList endpoint") return boot?.anilistEndpoint ?? "Loading";
    if (label === "Extension budget") {
      if (!boot) return "Loading";
      const mem = Math.round(boot.limits.extMemoryBytes / 1024 / 1024);
      return `${boot.limits.extTimeoutSecs}s calls, ${boot.limits.extResolveTimeoutSecs}s resolves, ${mem}MB per runtime`;
    }
    return fallback;
  }
</script>

<div class="p-6 md:p-8 max-w-[1440px] mx-auto">
  <div class="flex items-baseline gap-3 mb-1">
    <h1 class="text-[28px] font-bold" style="font-family: var(--font-display);">Settings</h1>
    <span class="text-sm" style="color: var(--paper-500);">everything here is real</span>
  </div>
  <p class="text-sm mb-6 max-w-2xl leading-relaxed" style="color: var(--paper-300);">
    Controls appear as they ship. Nothing on this page pretends to work before then.
  </p>

  <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
    {#each sections as section}
      <div class="p-5 rounded-2xl" style="background: var(--void-900); border: 1px solid var(--line);">
        <div class="flex items-center gap-2.5 mb-3">
          {@html section.icon}
          <h2 class="text-[15px] font-semibold">{section.title}</h2>
        </div>
        <dl>
          {#each section.rows as row, position}
            <div
              class="flex flex-col sm:flex-row sm:items-baseline gap-1 py-2.5"
              style={position > 0 ? "border-top: 1px solid var(--line);" : ""}
            >
              <dt class="text-sm sm:w-40 shrink-0" style="color: var(--paper-500);">{row.label}</dt>
              <dd class="text-sm break-all" style="color: var(--paper-50);">{rowValue(row.label, row.value)}</dd>
            </div>
          {/each}
        </dl>
      </div>
    {/each}
  </div>
</div>
