import { describe, expect, it } from "vitest";
import {
  pickStream,
  readQualityPreference,
  saveQualityPreference,
  streamHeight,
  type QualityPreference,
} from "./quality";

function stream(quality: string, url = quality) {
  return { quality, url, type: "hls" };
}

describe("quality preference", () => {
  it("reads a stored value and falls back to auto", () => {
    expect(readQualityPreference({ getItem: () => "720" })).toBe("720");
    expect(readQualityPreference({ getItem: () => "best" })).toBe("best");
    expect(readQualityPreference({ getItem: () => "4k" })).toBe("auto");
    expect(readQualityPreference({ getItem: () => null })).toBe("auto");
  });

  it("saves a value", () => {
    let saved = "";
    saveQualityPreference("480", { setItem: (_key, value) => (saved = value) });
    expect(saved).toBe("480");
  });

  it("parses common quality labels", () => {
    expect(streamHeight(stream("800p - HD-1 [Sub]"))).toBe(800);
    expect(streamHeight(stream("1920x1080"))).toBe(1080);
    expect(streamHeight(stream("1080p"))).toBe(1080);
    expect(streamHeight(stream("4K HDR"))).toBe(2160);
    expect(streamHeight(stream("HD-1 [Sub]"))).toBeNull();
  });

  it("keeps source order for auto", () => {
    const list = [stream("480p"), stream("1080p")];
    expect(pickStream(list, "auto")?.url).toBe("480p");
  });

  it("picks the highest for best", () => {
    const list = [stream("480p"), stream("1080p"), stream("720p")];
    expect(pickStream(list, "best")?.url).toBe("1080p");
  });

  it("picks an exact target, then the highest below it", () => {
    const list = [stream("800p"), stream("480p")];
    expect(pickStream(list, "480" as QualityPreference)?.url).toBe("480p");
    expect(pickStream(list, "720" as QualityPreference)?.url).toBe("480p");
    expect(pickStream(list, "1080" as QualityPreference)?.url).toBe("800p");
  });

  it("keeps the first stream's server and audio variant", () => {
    const list = [stream("800p - HD-1 [Sub]"), stream("480p - HD-1 [Dub]")];
    expect(pickStream(list, "720" as QualityPreference)?.url).toBe("800p - HD-1 [Sub]");
    expect(pickStream(list, "480" as QualityPreference)?.url).toBe("800p - HD-1 [Sub]");

    const sameVariant = [
      stream("1080p - HD-1 [Sub]"),
      stream("720p - HD-1 [Sub]"),
      stream("480p - HD-1 [Dub]"),
    ];
    expect(pickStream(sameVariant, "720" as QualityPreference)?.url).toBe("720p - HD-1 [Sub]");
  });

  it("falls back to the first stream when no height is available", () => {
    const list = [stream("HD-1"), stream("HD-2")];
    expect(pickStream(list, "1080" as QualityPreference)?.url).toBe("HD-1");
  });
});
