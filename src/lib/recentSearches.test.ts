import { describe, expect, it } from "vitest";
import {
  forgetRecentSearches,
  loadRecentSearches,
  MAX_RECENT_SEARCHES,
  RECENT_SEARCH_KEY,
  rememberSearch,
} from "./recentSearches";

function memoryStorage(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
    removeItem: (key: string) => void values.delete(key),
    values,
  };
}

describe("recent searches", () => {
  it("loads valid strings and caps the list", () => {
    const values = Array.from({ length: 12 }, (_, index) => `show ${index}`);
    const storage = memoryStorage({
      [RECENT_SEARCH_KEY]: JSON.stringify([...values, 7, null]),
    });
    expect(loadRecentSearches(storage)).toEqual(values.slice(0, MAX_RECENT_SEARCHES));
    expect(loadRecentSearches(null)).toEqual([]);
  });

  it("persists searches when not incognito", () => {
    const storage = memoryStorage();
    const next = rememberSearch("Popeye", ["Sailor Moon"], true, storage);
    expect(next).toEqual(["Popeye", "Sailor Moon"]);
    expect(storage.values.get(RECENT_SEARCH_KEY)).toBe(
      JSON.stringify(["Popeye", "Sailor Moon"]),
    );
  });

  it("keeps incognito searches in memory only", () => {
    const storage = memoryStorage();
    const next = rememberSearch("Private title", ["Old title"], false, storage);
    expect(next).toEqual(["Private title", "Old title"]);
    expect(storage.values.has(RECENT_SEARCH_KEY)).toBe(false);
  });

  it("clears the stored list", () => {
    const storage = memoryStorage({ [RECENT_SEARCH_KEY]: '["Old"]' });
    forgetRecentSearches(storage);
    expect(storage.values.has(RECENT_SEARCH_KEY)).toBe(false);
  });
});
