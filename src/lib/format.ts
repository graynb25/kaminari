// Small pure helpers for titles, covers and meta rows.

import type { Media } from "./types";

export function displayTitle(item: Media): string {
  return item.title.english || item.title.romaji || item.title.native || "Untitled";
}

export function coverOf(item: Media): string | null {
  return item.coverImage?.extraLarge || item.coverImage?.large || null;
}

export function cleanHtml(text?: string | null): string {
  if (!text) return "";
  return text
    .replace(/<br\s*\/?>/gi, " ")
    .replace(/<[^>]+>/g, "")
    .replace(/&quot;/g, '"')
    .replace(/&#039;/g, "'")
    .replace(/&amp;/g, "&")
    .replace(/\s+/g, " ")
    .trim();
}

export function metaOf(item: Media): string {
  const bits: string[] = [];
  if (item.seasonYear) bits.push(String(item.seasonYear));
  if (item.format) bits.push(item.format);
  if (item.episodes) bits.push(`${item.episodes} eps`);
  return bits.join("  ·  ");
}

export function isRateLimitMessage(message: string): boolean {
  const lower = message.toLowerCase();
  return lower.includes("429") || lower.includes("rate") || lower.includes("retry-after");
}
