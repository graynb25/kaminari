import { describe, expect, it } from "vitest";
import { normalizeTitle, sortByMatch, titleScore } from "./sourceMatch";

function item(title: string, id = title) {
  return { id, title, cover: null };
}

describe("source title matching", () => {
  it("normalizes punctuation and case", () => {
    expect(normalizeTitle("Frieren: Beyond Journey's End")).toBe(
      "frieren beyond journey s end",
    );
  });

  it("scores exact, prefix and token matches", () => {
    expect(titleScore("Naruto", "Naruto")).toBe(100);
    expect(titleScore("Naruto Shippuden", "Naruto")).toBe(80);
    expect(titleScore("Naruto: Shippuden", "Naruto Shippuden")).toBe(100);
    expect(titleScore("One Piece", "Naruto")).toBe(0);
    expect(titleScore("Frieren: Beyond Journey's End", "Frieren")).toBe(80);
  });

  it("puts the closest result first", () => {
    const sorted = sortByMatch(
      [item("Naruto the Movie"), item("Naruto"), item("Boruto")],
      "Naruto",
    );
    expect(sorted.map((row) => row.title)).toEqual([
      "Naruto",
      "Naruto the Movie",
      "Boruto",
    ]);
  });

  it("keeps source order when scores tie", () => {
    const sorted = sortByMatch([item("A"), item("B")], "Naruto");
    expect(sorted.map((row) => row.title)).toEqual(["A", "B"]);
  });
});
