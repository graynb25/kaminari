import { describe, expect, it } from "vitest";
import { mergeContinueRows } from "./continueRows";
import { normalizeTitle } from "./format";
import type { ContinueItem, SourceContinueItem } from "./types";

function anilist(id: number, title: string, updatedAt: number): ContinueItem {
  return {
    media: { id, title: { romaji: title, english: title } },
    episode: 1,
    positionSec: 12,
    durationSec: 100,
    watchedCount: 1,
    totalEpisodes: 12,
    updatedAt,
  };
}

function source(
  title: string,
  showId: string,
  updatedAt: number,
  anilistId: number | null = null,
): SourceContinueItem {
  return {
    extensionId: "ext",
    extensionName: "Source",
    showId,
    title,
    anilistId,
    episodeId: `${showId}-1`,
    episodeNumber: 1,
    positionSec: 10,
    durationSec: 100,
    watched: false,
    updatedAt,
  };
}

describe("continue rows", () => {
  it("normalizes punctuation, case and accents", () => {
    expect(normalizeTitle("Mob Psycho 100!!")).toBe("mob psycho 100");
    expect(normalizeTitle("Pokémon")).toBe("pokemon");
  });

  it("merges by durable AniList id even when titles differ", () => {
    const rows = mergeContinueRows(
      [anilist(7, "Frieren: Beyond Journey's End", 200)],
      [source("Frieren", "show-1", 100, 7)],
    );
    expect(rows).toHaveLength(1);
    const row = rows[0];
    expect(row.kind).toBe("linked");
    if (row.kind !== "linked") return;
    expect(row.suggested).toBe(false);
    expect(row.anilist.media.id).toBe(7);
    expect(row.source.showId).toBe("show-1");
  });

  it("prefers the most recently updated source when a show has several", () => {
    const rows = mergeContinueRows(
      [anilist(7, "Frieren", 200)],
      [
        source("Frieren", "show-old", 50, 7),
        source("Frieren", "show-new", 300, 7),
      ],
    );
    expect(rows).toHaveLength(1);
    const row = rows[0];
    expect(row.kind).toBe("linked");
    if (row.kind !== "linked") return;
    expect(row.source.showId).toBe("show-new");
    expect(row.at).toBe(300);
  });

  it("keeps a linked source row when AniList has no Continue row", () => {
    const rows = mergeContinueRows([], [source("Frieren", "show-1", 100, 7)]);
    expect(rows).toHaveLength(1);
    expect(rows[0].kind).toBe("source");
  });

  it("uses title matching only as a marked suggestion for unlinked rows", () => {
    const rows = mergeContinueRows(
      [anilist(1, "Naruto", 200)],
      [source("Naruto", "show-1", 100)],
    );
    expect(rows).toHaveLength(1);
    const row = rows[0];
    expect(row.kind).toBe("linked");
    if (row.kind !== "linked") return;
    expect(row.suggested).toBe(true);
    expect(row.source.showId).toBe("show-1");
  });

  it("does not title-match a source that already has a different AniList id", () => {
    const rows = mergeContinueRows(
      [anilist(1, "Naruto", 200)],
      [source("Naruto", "show-1", 100, 99)],
    );
    expect(rows.map((row) => row.kind)).toEqual(["anilist", "source"]);
  });

  it("matches against any AniList title", () => {
    const item = anilist(1, "English title", 200);
    item.media.title.romaji = "Romaji title";
    const rows = mergeContinueRows([item], [source("romaji title", "show-1", 100)]);
    expect(rows).toHaveLength(1);
    expect(rows[0].kind).toBe("linked");
  });

  it("keeps different titles and sorts by newest", () => {
    const rows = mergeContinueRows(
      [anilist(1, "Naruto", 100)],
      [source("Mob Psycho 100", "show-2", 300)],
    );
    expect(rows.map((row) => row.kind)).toEqual(["source", "anilist"]);
  });
});
