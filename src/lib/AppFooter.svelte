<script lang="ts">
  import Icon from "./Icon.svelte";
  import LegalDialog from "./LegalDialog.svelte";

  let { onOpenLicenses }: { onOpenLicenses: () => void } = $props();

  let legal = $state<"terms" | "privacy" | null>(null);
</script>

<footer class="app-footer" aria-label="Footer">
  <div class="app-footer-inner">
    <div class="flex items-center gap-2.5 min-w-0">
      <span class="footer-mark" aria-hidden="true">
        <Icon name="bolt" size={13} />
      </span>
      <div class="min-w-0">
        <p class="text-xs font-semibold">Kaminari</p>
        <p class="footer-copy">No content or sources are included.</p>
      </div>
    </div>

    <div class="footer-links">
      <span class="footer-meta">Metadata by AniList</span>
      <span class="footer-sep" aria-hidden="true"></span>
      <button class="footer-link" onclick={() => (legal = "terms")} aria-haspopup="dialog">
        Terms
      </button>
      <button class="footer-link" onclick={() => (legal = "privacy")} aria-haspopup="dialog">
        Privacy
      </button>
      <button class="footer-link" onclick={onOpenLicenses} aria-haspopup="dialog">
        Licenses
      </button>
      <span class="footer-version">{__APP_VERSION__}</span>
    </div>
  </div>
</footer>

{#if legal}
  <LegalDialog initial={legal} onClose={() => (legal = null)} />
{/if}

<style>
  .app-footer {
    margin-top: auto;
    border-top: 1px solid var(--line);
    background: var(--void-925);
  }

  .app-footer-inner {
    max-width: 1440px;
    margin: 0 auto;
    padding: 16px 24px 18px;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 14px 20px;
  }

  .footer-mark {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    border-radius: 9px;
    background: linear-gradient(180deg, var(--accent-wash-strong), var(--accent-wash));
    border: 1px solid var(--line-volt);
    color: var(--volt-300);
  }

  .footer-copy {
    font-size: 10px;
    line-height: 1.4;
    color: var(--text-4);
    margin-top: 1px;
  }

  .footer-links {
    margin-left: auto;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
  }

  .footer-meta,
  .footer-version {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-4);
  }

  .footer-sep {
    width: 1px;
    height: 14px;
    background: var(--line-strong);
  }

  .footer-link {
    height: 26px;
    padding: 0 8px;
    border-radius: 7px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-3);
    transition:
      background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .footer-link:hover {
    background: var(--tint-1);
    color: var(--text-1);
  }

  .footer-link:focus-visible {
    outline: 2px solid var(--volt-400);
    outline-offset: 1px;
  }

  @media (min-width: 768px) {
    .app-footer-inner {
      padding-left: 32px;
      padding-right: 32px;
    }
  }
</style>
