import { describe, expect, it } from "vitest";
import { nextFocusIndex } from "./a11y";

describe("focus order", () => {
  it("wraps forward and backward", () => {
    expect(nextFocusIndex(0, 3, 1)).toBe(1);
    expect(nextFocusIndex(2, 3, 1)).toBe(0);
    expect(nextFocusIndex(0, 3, -1)).toBe(2);
    expect(nextFocusIndex(1, 3, -1)).toBe(0);
  });

  it("starts at the first or last item when nothing is focused", () => {
    expect(nextFocusIndex(-1, 3, 1)).toBe(0);
    expect(nextFocusIndex(-1, 3, -1)).toBe(2);
  });

  it("returns no index for an empty list", () => {
    expect(nextFocusIndex(0, 0, 1)).toBe(-1);
  });
});
