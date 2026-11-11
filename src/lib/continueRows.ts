import { displayTitle, normalizeTitle } from "./format";
import type { ContinueItem, SourceContinueItem } from "./types";

export type ContinueRow =
  | { kind: "anilist"; at: number; item: ContinueItem }
  | { kind: "source"; at: number; item: SourceContinueItem }
  | {
      kind: "linked";
      at: number;
      anilist: ContinueItem;
      source: SourceContinueItem;
      suggested: boolean;
    };

function anilistTitles(item: ContinueItem): string[] {
  return [
    displayTitle(item.media),
    item.media.title.english,
    item.media.title.romaji,
    item.media.title.native,
  ]
    .filter((value): value is string => Boolean(value))
    .map(normalizeTitle);
}

/**
 * Merge the two Continue Watching stores.
 *
 * A durable source_library.anilist_id is the identity. When several source
 * shows point at the same AniList title, the most recently updated source row
 * is the resume target and the others are folded into it. Title matching is
 * only a fallback for rows without a link, and the result is marked as a
 * suggestion so it is never treated as a durable link.
 */
export function mergeContinueRows(
  anilist: ContinueItem[],
  source: SourceContinueItem[],
): ContinueRow[] {
  const orderedSources = [...source].sort((a, b) => b.updatedAt - a.updatedAt);
  const preferredByAniList = new Map<number, SourceContinueItem>();
  for (const item of orderedSources) {
    if (item.anilistId == null) continue;
    if (!preferredByAniList.has(item.anilistId)) {
      preferredByAniList.set(item.anilistId, item);
    }
  }

  const rows: ContinueRow[] = [];
  const usedAniListIds = new Set<number>();
  const unmatchedAniList: ContinueItem[] = [];

  for (const item of anilist) {
    const linked = preferredByAniList.get(item.media.id);
    if (linked) {
      usedAniListIds.add(item.media.id);
      rows.push({
        kind: "linked",
        at: Math.max(item.updatedAt, linked.updatedAt),
        anilist: item,
        source: linked,
        suggested: false,
      });
    } else {
      unmatchedAniList.push(item);
    }
  }

  // A linked source show with no AniList Continue row still belongs on the
  // shelf. The source row is the only available resume target then.
  for (const [anilistId, item] of preferredByAniList) {
    if (usedAniListIds.has(anilistId)) continue;
    rows.push({ kind: "source", at: item.updatedAt, item });
  }

  const byTitle = new Map<string, ContinueItem[]>();
  for (const item of unmatchedAniList) {
    for (const title of anilistTitles(item)) {
      const bucket = byTitle.get(title) ?? [];
      bucket.push(item);
      byTitle.set(title, bucket);
    }
  }

  const suggestedAniList = new Set<ContinueItem>();
  const suggestedSource = new Set<SourceContinueItem>();
  for (const sourceItem of orderedSources) {
    if (sourceItem.anilistId != null) continue;
    const key = normalizeTitle(sourceItem.title);
    const candidate = (byTitle.get(key) ?? []).find(
      (item) => !suggestedAniList.has(item),
    );
    if (!candidate) continue;
    suggestedAniList.add(candidate);
    suggestedSource.add(sourceItem);
    rows.push({
      kind: "linked",
      at: Math.max(candidate.updatedAt, sourceItem.updatedAt),
      anilist: candidate,
      source: sourceItem,
      suggested: true,
    });
  }

  for (const item of orderedSources) {
    if (item.anilistId != null) continue;
    if (suggestedSource.has(item)) continue;
    rows.push({ kind: "source", at: item.updatedAt, item });
  }

  for (const item of unmatchedAniList) {
    if (suggestedAniList.has(item)) continue;
    rows.push({ kind: "anilist", at: item.updatedAt, item });
  }

  return rows.sort((a, b) => b.at - a.at);
}
