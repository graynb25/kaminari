import { describe, expect, it } from "vitest";
import { mergeContinueRows, normalizeContinueTitle } from "./continueRows";
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

function source(title: string, showId: string, updatedAt: number): SourceContinueItem {
  return {
    extensionId: "ext",
    extensionName: "Source",
    showId,
    title,
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
    expect(normalizeContinueTitle("Mob Psycho 100!!")).toBe("mob psycho 100");
    expect(normalizeContinueTitle("Pokémon")).toBe("pokemon");
  });

  it("drops an AniList row when a source row has the same title", () => {
    const rows = mergeContinueRows(
      [anilist(1, "Naruto", 200)],
      [source("Naruto", "show-1", 100)],
    );
    expect(rows).toHaveLength(1);
    expect(rows[0].kind).toBe("source");
  });

  it("matches against any AniList title", () => {
    const item = anilist(1, "English title", 200);
    item.media.title.romaji = "Romaji title";
    const rows = mergeContinueRows([item], [source("romaji title", "show-1", 100)]);
    expect(rows).toHaveLength(1);
    expect(rows[0].kind).toBe("source");
  });

  it("keeps different titles and sorts by newest", () => {
    const rows = mergeContinueRows(
      [anilist(1, "Naruto", 100)],
      [source("Mob Psycho 100", "show-2", 300)],
    );
    expect(rows.map((row) => row.kind)).toEqual(["source", "anilist"]);
  });
});
