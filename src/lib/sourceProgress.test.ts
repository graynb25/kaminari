import { describe, expect, it } from "vitest";
import { anilistProgressThrough, sourceProgressMarks } from "./sourceProgress";
import type { JsEpisode } from "./types";

const episodes: JsEpisode[] = [
  { id: "ep-1", number: 1, title: "Start", durationSec: 1440 },
  { id: "ep-2", number: 2, title: null, durationSec: null },
  { id: "ep-3", number: 3 },
];

describe("sourceProgressMarks", () => {
  it("maps source episodes into progress rows", () => {
    expect(sourceProgressMarks(episodes)).toEqual([
      { episodeId: "ep-1", episodeNumber: 1, episodeTitle: "Start", durationSec: 1440 },
      { episodeId: "ep-2", episodeNumber: 2, episodeTitle: null, durationSec: 0 },
      { episodeId: "ep-3", episodeNumber: 3, episodeTitle: null, durationSec: 0 },
    ]);
  });

  it("handles an empty episode list", () => {
    expect(sourceProgressMarks([])).toEqual([]);
  });
});

describe("anilistProgressThrough", () => {
  it("uses the highest source episode when AniList has no total", () => {
    expect(anilistProgressThrough(null, episodes)).toBe(3);
    expect(anilistProgressThrough(0, episodes)).toBe(3);
  });

  it("clamps to the AniList episode total", () => {
    expect(anilistProgressThrough(2, episodes)).toBe(2);
    expect(anilistProgressThrough(24, episodes)).toBe(3);
  });

  it("returns zero when the source has no numbered episodes", () => {
    expect(anilistProgressThrough(12, [])).toBe(0);
    expect(anilistProgressThrough(12, [{ id: "special", number: 0 }])).toBe(0);
  });
});
