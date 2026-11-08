import { describe, expect, it } from "vitest";
import { artworkStatus, isArtworkBroken, isArtworkComplete } from "./artwork";

describe("artworkStatus", () => {
  it("is empty without a source", () => {
    expect(artworkStatus(null, false, false)).toBe("empty");
    expect(artworkStatus(undefined, false, false)).toBe("empty");
    expect(artworkStatus("", false, false)).toBe("empty");
  });

  it("is failed after an error", () => {
    expect(artworkStatus("https://img.example/a.jpg", false, true)).toBe("failed");
    expect(artworkStatus("https://img.example/a.jpg", true, true)).toBe("failed");
  });

  it("moves from loading to ready on decode", () => {
    expect(artworkStatus("https://img.example/a.jpg", false, false)).toBe("loading");
    expect(artworkStatus("https://img.example/a.jpg", true, false)).toBe("ready");
  });

  it("prefers empty when the source is missing even if flagged", () => {
    expect(artworkStatus(null, true, true)).toBe("empty");
  });
});

describe("complete check", () => {
  it("treats complete images with width as cached", () => {
    expect(isArtworkComplete({ complete: true, naturalWidth: 120 })).toBe(true);
    expect(isArtworkComplete({ complete: false, naturalWidth: 120 })).toBe(false);
    expect(isArtworkComplete({ complete: true, naturalWidth: 0 })).toBe(false);
  });

  it("treats complete images with no width as broken", () => {
    expect(isArtworkBroken({ complete: true, naturalWidth: 0 })).toBe(true);
    expect(isArtworkBroken({ complete: false, naturalWidth: 0 })).toBe(false);
    expect(isArtworkBroken({ complete: true, naturalWidth: 40 })).toBe(false);
  });
});
