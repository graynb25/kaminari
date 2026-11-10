import { displayTitle } from "./format";
import type { ContinueItem, SourceContinueItem } from "./types";

export type ContinueRow =
  | { kind: "anilist"; at: number; item: ContinueItem }
  | { kind: "source"; at: number; item: SourceContinueItem };

export function normalizeContinueTitle(title: string): string {
  return title
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

function anilistTitles(item: ContinueItem): string[] {
  return [
    displayTitle(item.media),
    item.media.title.english,
    item.media.title.romaji,
    item.media.title.native,
  ]
    .filter((value): value is string => Boolean(value))
    .map(normalizeContinueTitle);
}

export function mergeContinueRows(
  anilist: ContinueItem[],
  source: SourceContinueItem[],
): ContinueRow[] {
  const sourceTitles = new Set(
    source.map((item) => normalizeContinueTitle(item.title)).filter(Boolean),
  );
  const rows: ContinueRow[] = [];

  for (const item of anilist) {
    if (anilistTitles(item).some((title) => sourceTitles.has(title))) continue;
    rows.push({ kind: "anilist", at: item.updatedAt, item });
  }
  for (const item of source) {
    rows.push({ kind: "source", at: item.updatedAt, item });
  }

  return rows.sort((a, b) => b.at - a.at);
}
