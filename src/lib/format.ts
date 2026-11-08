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

// AniList returns 0 to 100. We show a 10 point score with one decimal.
// A star next to 87 reads like 87 stars, which is not the scale.
export function scoreOutOfTen(score?: number | null): string {
  if (score == null || score <= 0) return "";
  return (score / 10).toFixed(1);
}

export function scoreLabel(score?: number | null): string {
  const value = scoreOutOfTen(score);
  return value ? `${value} / 10` : "n/a";
}

export function isRateLimitMessage(message: string): boolean {
  const lower = message.toLowerCase();
  return lower.includes("429") || lower.includes("rate") || lower.includes("retry-after");
}

export function timeAgo(unixSeconds: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = Math.max(0, now - unixSeconds);
  if (diff < 60) return "just now";
  const minutes = Math.floor(diff / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return new Date(unixSeconds * 1000).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}
