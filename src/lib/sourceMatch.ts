// Match an AniList title to a source's search results. The source still owns
// the final list; this only puts the closest names first.

import { normalizeTitle } from "./format";
import type { JsSearchItem } from "./types";

export function titleScore(candidate: string, query: string): number {
  const left = normalizeTitle(candidate);
  const right = normalizeTitle(query);
  if (!left || !right) return 0;
  if (left === right) return 100;
  if (left.startsWith(right) || right.startsWith(left)) return 80;
  if (left.includes(right) || right.includes(left)) return 60;

  const leftTokens = new Set(left.split(" ").filter(Boolean));
  const rightTokens = new Set(right.split(" ").filter(Boolean));
  let overlap = 0;
  for (const token of rightTokens) {
    if (leftTokens.has(token)) overlap += 1;
  }
  return Math.round((overlap / Math.max(1, rightTokens.size)) * 50);
}

export function sortByMatch(items: JsSearchItem[], query: string): JsSearchItem[] {
  return items
    .map((item, index) => ({ item, index, score: titleScore(item.title, query) }))
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((row) => row.item);
}
