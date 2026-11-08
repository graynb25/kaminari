<script lang="ts">
  import Icon from "./Icon.svelte";
  import { isRateLimitMessage } from "./format";
  import PosterCard from "./PosterCard.svelte";
  import Shelf from "./Shelf.svelte";
  import Skeleton from "./Skeleton.svelte";
  import type { Media, RailState } from "./types";

  let {
    title,
    state,
    rank = false,
    isInLibrary,
    onOpen,
    onToggleLibrary,
    onPlay,
    onRetry,
  }: {
    title: string;
    state: RailState;
    rank?: boolean;
    isInLibrary: (id: number) => boolean;
    onOpen: (item: Media) => void;
    onToggleLibrary: (item: Media) => void;
    onPlay: (item: Media) => void;
    onRetry: () => void;
  } = $props();

  const shown = $derived(rank ? state.items.slice(0, 10) : state.items);
</script>

<section aria-label={title}>
  <Shelf
    title={title}
    count={rank ? shown.length : state.items.length}
    resetKey={state.loading ? "loading" : `${state.items.length}-${state.items[0]?.id ?? 0}`}
  >
    {#if state.loading}
      <Skeleton kind="row" />
    {:else if state.error}
      <div class="w-full surface px-4 py-3.5 flex flex-col sm:flex-row sm:items-center gap-3 text-sm" role="alert">
        <span class="w-8 h-8 grid place-items-center rounded-lg shrink-0" style="background: var(--warning-wash); color: var(--warning);">
          <Icon name="alert" size={16} />
        </span>
        <p class="flex-1" style="color: var(--paper-300);">
          {isRateLimitMessage(state.error)
            ? `${title} hit the AniList rate limit. It clears in under a minute.`
            : `${title} did not load.`}
        </p>
        <button class="btn-ghost px-3 py-1.5 text-xs shrink-0" onclick={onRetry}>Retry</button>
      </div>
    {:else if shown.length === 0}
      <div class="w-full surface px-4 py-3.5 text-sm" style="color: var(--paper-500);">
        No titles in this shelf yet.
      </div>
    {:else if rank}
      {#each shown as item, position (item.id)}
        <div
          class="flex items-end shrink-0"
          style={position === 0 ? "padding-left: 18px;" : ""}
          aria-label="Rank {position + 1}: {item.title.english || item.title.romaji || 'Untitled'}"
        >
          <span class="rank-num {position < 3 ? 'rank-hot' : ''}" aria-hidden="true">{String(position + 1).padStart(2, "0")}</span>
          <div class="w-[164px] lg:w-[184px] -ml-5 mb-1">
            <PosterCard
              item={item}
              inLibrary={isInLibrary(item.id)}
              onOpen={onOpen}
              onToggleLibrary={onToggleLibrary}
              onPlay={onPlay}
            />
          </div>
        </div>
      {/each}
    {:else}
      {#each shown as item (item.id)}
        <div class="w-[164px] lg:w-[184px] shrink-0">
          <PosterCard
            item={item}
            inLibrary={isInLibrary(item.id)}
            onOpen={onOpen}
            onToggleLibrary={onToggleLibrary}
            onPlay={onPlay}
          />
        </div>
      {/each}
    {/if}
  </Shelf>
</section>
