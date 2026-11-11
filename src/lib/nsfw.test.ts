import { describe, expect, it } from "vitest";
import {
  filterNsfwSources,
  SHOW_NSFW_KEY,
  setShowNsfwEnabled,
  showNsfwEnabled,
} from "./nsfw";

function memoryStorage(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
    values,
  };
}

describe("NSFW source setting", () => {
  it("defaults to hidden", () => {
    expect(showNsfwEnabled(memoryStorage())).toBe(false);
    expect(showNsfwEnabled(null)).toBe(false);
  });

  it("reads both stored true forms", () => {
    expect(showNsfwEnabled(memoryStorage({ [SHOW_NSFW_KEY]: "true" }))).toBe(true);
    expect(showNsfwEnabled(memoryStorage({ [SHOW_NSFW_KEY]: "1" }))).toBe(true);
  });

  it("stores off and on", () => {
    const storage = memoryStorage();
    setShowNsfwEnabled(true, storage);
    expect(storage.values.get(SHOW_NSFW_KEY)).toBe("true");
    expect(showNsfwEnabled(storage)).toBe(true);
    setShowNsfwEnabled(false, storage);
    expect(storage.values.get(SHOW_NSFW_KEY)).toBe("false");
    expect(showNsfwEnabled(storage)).toBe(false);
  });

  it("filters installed and browsed sources when off", () => {
    const sources = [
      { id: "safe", nsfw: false },
      { id: "adult", nsfw: true },
    ];
    expect(filterNsfwSources(sources, false).map((item) => item.id)).toEqual(["safe"]);
    expect(filterNsfwSources(sources, true).map((item) => item.id)).toEqual([
      "safe",
      "adult",
    ]);
  });
});
