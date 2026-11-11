import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanHtml,
  coverOf,
  displayTitle,
  isRateLimitMessage,
  metaOf,
  normalizeTitle,
  scoreLabel,
  scoreOutOfTen,
  timeAgo,
} from "./format";
import type { Media } from "./types";

function media(overrides: Partial<Media> = {}): Media {
  return {
    id: 1,
    title: { romaji: "Sousou no Frieren", english: "Frieren: Beyond Journey's End" },
    coverImage: { extraLarge: "https://img.example/frieren-xl.jpg", large: "https://img.example/frieren.jpg" },
    bannerImage: null,
    description: null,
    genres: null,
    averageScore: 91,
    seasonYear: 2023,
    format: "TV",
    status: "FINISHED",
    episodes: 28,
    ...overrides,
  };
}

afterEach(() => {
  vi.useRealTimers();
});

describe("displayTitle", () => {
  it("prefers the English title", () => {
    expect(displayTitle(media())).toBe("Frieren: Beyond Journey's End");
  });

  it("falls back to romaji, then native, then Untitled", () => {
    expect(displayTitle(media({ title: { romaji: "Sousou no Frieren" } }))).toBe("Sousou no Frieren");
    expect(displayTitle(media({ title: { native: "葬送のフリーレン" } }))).toBe("葬送のフリーレン");
    expect(displayTitle(media({ title: {} }))).toBe("Untitled");
  });
});

describe("coverOf", () => {
  it("prefers the extra large cover", () => {
    expect(coverOf(media())).toBe("https://img.example/frieren-xl.jpg");
  });

  it("falls back to the large cover and returns null for no art", () => {
    expect(coverOf(media({ coverImage: { large: "https://img.example/frieren.jpg" } }))).toBe(
      "https://img.example/frieren.jpg",
    );
    expect(coverOf(media({ coverImage: null }))).toBeNull();
  });
});

describe("normalizeTitle", () => {
  it("folds case, punctuation and accents into comparable words", () => {
    expect(normalizeTitle("Frieren: Beyond Journey's End")).toBe(
      "frieren beyond journey s end",
    );
    expect(normalizeTitle("Pokémon")).toBe("pokemon");
    expect(normalizeTitle("  Mob   Psycho 100!! ")).toBe("mob psycho 100");
    expect(normalizeTitle("")).toBe("");
  });
});

describe("cleanHtml", () => {
  it("strips tags, decodes entities and collapses whitespace", () => {
    expect(cleanHtml("<p>A &amp; B</p><br>  next &quot;line&quot;")).toBe('A & B next "line"');
    expect(cleanHtml("It&#039;s fine")).toBe("It's fine");
    expect(cleanHtml("a &lt;tag&gt; &amp;amp; b")).toBe("a <tag> &amp; b");
  });

  it("returns an empty string for missing text", () => {
    expect(cleanHtml(null)).toBe("");
    expect(cleanHtml(undefined)).toBe("");
  });
});

describe("metaOf", () => {
  it("joins year, format and episode count", () => {
    expect(metaOf(media())).toBe("2023  ·  TV  ·  28 eps");
  });

  it("returns an empty string when there is nothing to show", () => {
    expect(metaOf(media({ seasonYear: null, format: null, episodes: null }))).toBe("");
  });
});

describe("scores", () => {
  it("converts a 0 to 100 score to one decimal on a ten point scale", () => {
    expect(scoreOutOfTen(87)).toBe("8.7");
    expect(scoreOutOfTen(100)).toBe("10.0");
  });

  it("returns an empty value for a missing or zero score", () => {
    expect(scoreOutOfTen(null)).toBe("");
    expect(scoreOutOfTen(0)).toBe("");
    expect(scoreOutOfTen(-1)).toBe("");
  });

  it("labels a score or n/a", () => {
    expect(scoreLabel(87)).toBe("8.7 / 10");
    expect(scoreLabel(null)).toBe("n/a");
  });
});

describe("isRateLimitMessage", () => {
  it("recognises the common rate limit forms", () => {
    expect(isRateLimitMessage("429 Too Many Requests")).toBe(true);
    expect(isRateLimitMessage("AniList rate limit reached")).toBe(true);
    expect(isRateLimitMessage("Retry-After: 30")).toBe(true);
  });

  it("does not flag unrelated failures", () => {
    expect(isRateLimitMessage("network unavailable")).toBe(false);
  });
});

describe("timeAgo", () => {
  it("formats recent times in words", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-08T12:00:00Z"));
    const now = Math.floor(Date.now() / 1000);
    expect(timeAgo(now - 30)).toBe("just now");
    expect(timeAgo(now - 120)).toBe("2m ago");
    expect(timeAgo(now - 7200)).toBe("2h ago");
    expect(timeAgo(now - 3 * 86400)).toBe("3d ago");
  });

  it("falls back to a date after a week", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-08T12:00:00Z"));
    const now = Math.floor(Date.now() / 1000);
    const label = timeAgo(now - 30 * 86400);
    expect(label).not.toContain("ago");
    expect(label.length).toBeGreaterThan(0);
  });
});
