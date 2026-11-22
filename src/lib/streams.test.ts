import { describe, expect, it } from "vitest";
import {
  audioMasterFor,
  collapseHlsVariants,
  isMasterCandidate,
  uniqueStreams,
} from "./streams";
import type { JsStream } from "./types";

const MASTER = "https://cdn.example.com/master.m3u8";

function variant(
  quality: string,
  url: string,
  originalUrl: string | null = MASTER,
): JsStream {
  return { url, originalUrl, type: "hls", quality };
}

describe("streams master handling", () => {
  it("collapses two variants into one auto master", () => {
    const list = [
      variant("1080p [Sub]", "https://cdn.example.com/index-f1.m3u8"),
      variant("720p [Sub]", "https://cdn.example.com/index-f2.m3u8"),
    ];
    const collapsed = collapseHlsVariants(list);
    expect(collapsed).toHaveLength(1);
    expect(collapsed[0].url).toBe(MASTER);
    expect(collapsed[0].quality).toBe("Auto");
    expect(collapsed[0].originalUrl).toBeNull();
  });

  it("leaves a lone stream with originalUrl untouched", () => {
    const list = [variant("1080p", "https://cdn.example.com/index-f1.m3u8")];
    expect(isMasterCandidate(list[0])).toBe(true);
    const collapsed = collapseHlsVariants(list);
    expect(collapsed).toHaveLength(1);
    expect(collapsed[0].url).toBe(list[0].url);
  });

  it("collapses sub and dub duplicates without duplicate menu entries", () => {
    const list = [
      variant("1080p [Sub]", "https://cdn.example.com/index-f1.m3u8"),
      variant("720p [Sub]", "https://cdn.example.com/index-f2.m3u8"),
      variant("360p [Sub]", "https://cdn.example.com/index-f3.m3u8"),
      variant("1080p [Dub]", "https://cdn.example.com/index-f1.m3u8"),
      variant("720p [Dub]", "https://cdn.example.com/index-f2.m3u8"),
      variant("360p [Dub]", "https://cdn.example.com/index-f3.m3u8"),
    ];
    const collapsed = collapseHlsVariants(list);
    expect(collapsed).toHaveLength(1);
    expect(collapsed[0].url).toBe(MASTER);
    const urls = collapsed.map((row) => row.url);
    expect(new Set(urls).size).toBe(urls.length);
  });

  it("leaves non-HLS streams untouched", () => {
    const list: JsStream[] = [
      { url: "https://cdn.example.com/a.mp4", type: "mp4", quality: "1080p" },
      { url: "https://cdn.example.com/b.mp4", type: "mp4", quality: "720p" },
    ];
    expect(collapseHlsVariants(list)).toEqual(list);
    expect(isMasterCandidate(list[0])).toBe(false);
  });

  it("returns the master for a grouped variant and null for an ungrouped one", () => {
    const grouped = [
      variant("1080p [Sub]", "https://cdn.example.com/index-f1.m3u8"),
      variant("720p [Sub]", "https://cdn.example.com/index-f2.m3u8"),
    ];
    const master = audioMasterFor(grouped[0], grouped);
    expect(master?.url).toBe(MASTER);
    expect(master?.masterPlaylist).toBe(true);
    const lone = [variant("1080p", "https://cdn.example.com/index-f1.m3u8")];
    expect(audioMasterFor(lone[0], lone)).toBeNull();
  });

  it("marks only the synthetic master as a master playlist", () => {
    const grouped = [
      variant("1080p [Sub]", "https://cdn.example.com/index-f1.m3u8"),
      variant("720p [Sub]", "https://cdn.example.com/index-f2.m3u8"),
    ];
    expect(grouped[0].masterPlaylist).toBeUndefined();
    const collapsed = collapseHlsVariants(grouped);
    expect(collapsed[0].masterPlaylist).toBe(true);
  });

  it("maps dub to en and sub to ja", () => {
    const pair = [
      variant("1080p [Dub]", "https://cdn.example.com/index-f1.m3u8"),
      variant("720p [Dub]", "https://cdn.example.com/index-f2.m3u8"),
    ];
    expect(audioMasterFor(pair[0], pair)?.audioLanguage).toBe("en");
    const subPair = [
      variant("1080p [Sub]", "https://cdn.example.com/index-f1.m3u8"),
      variant("720p [Sub]", "https://cdn.example.com/index-f2.m3u8"),
    ];
    expect(audioMasterFor(subPair[0], subPair)?.audioLanguage).toBe("ja");
  });

  it("dedupes exact duplicate urls", () => {
    const list: JsStream[] = [
      { url: "https://cdn.example.com/a.mp4", type: "mp4" },
      { url: "https://cdn.example.com/a.mp4", type: "mp4" },
    ];
    expect(uniqueStreams(list)).toHaveLength(1);
  });
});
