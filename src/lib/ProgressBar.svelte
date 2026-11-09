<script lang="ts">
  type Tone = "accent" | "success" | "warning" | "danger";
  type Size = "xs" | "sm" | "md";

  let {
    value = null,
    max = 100,
    label,
    indeterminate = false,
    tone = "accent",
    size = "sm",
    class: className = "",
  }: {
    value?: number | null;
    max?: number;
    label: string;
    indeterminate?: boolean;
    tone?: Tone;
    size?: Size;
    class?: string;
  } = $props();

  const safeMax = $derived(max > 0 ? max : 100);
  const current = $derived(
    value == null || !Number.isFinite(value) ? 0 : Math.min(safeMax, Math.max(0, value)),
  );
  const percent = $derived((current / safeMax) * 100);
</script>

<div
  class="progress progress-{size} {className}"
  class:is-indeterminate={indeterminate}
  data-tone={tone}
  role="progressbar"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={safeMax}
  aria-valuenow={indeterminate ? undefined : current}
>
  <span class="progress-fill" style="--progress: {percent / 100};"></span>
</div>

<style>
  .progress {
    position: relative;
    width: 100%;
    overflow: hidden;
    border-radius: 999px;
    background: var(--track);
    --progress-tone: var(--volt-400);
  }

  .progress-xs {
    height: 2px;
  }

  .progress-sm {
    height: 3px;
  }

  .progress-md {
    height: 4px;
  }

  .progress[data-tone="success"] {
    --progress-tone: var(--success);
  }

  .progress[data-tone="warning"] {
    --progress-tone: var(--warning);
  }

  .progress[data-tone="danger"] {
    --progress-tone: var(--danger);
  }

  .progress-fill {
    display: block;
    width: 100%;
    height: 100%;
    background: var(--progress-tone);
    border-radius: inherit;
    transform: scaleX(var(--progress, 0));
    transform-origin: left center;
  }

  .progress.is-indeterminate .progress-fill {
    animation: progressSweep 1200ms var(--ease-out) infinite;
  }

  @keyframes progressSweep {
    from {
      transform: translateX(-100%) scaleX(0.35);
    }
    to {
      transform: translateX(100%) scaleX(0.35);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .progress.is-indeterminate .progress-fill {
      animation: none;
      transform: scaleX(0.35);
    }
  }
</style>
