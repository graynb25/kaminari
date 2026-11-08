import { describe, expect, it } from "vitest";
import {
  isMp4Stream,
  playbackUrl,
  proxyFlagEnabled,
  shouldProxy,
} from "./streamProxy";
import type { JsStream } from "./types";

function stream(overrides: Partial<JsStream> = {}): JsStream {
  return {
    url: "https://cdn.example/show/ep1.mp4",
    type: "mp4",
    ...overrides,
  };
}

describe("shouldProxy", () => {
  it("proxies MP4 streams with headers", () => {
    expect(shouldProxy(stream({ headers: { Referer: "https://source.example/" } }))).toBe(true);
  });

  it("leaves headerless MP4 on the direct path", () => {
    expect(shouldProxy(stream({ headers: null }))).toBe(false);
    expect(shouldProxy(stream({ headers: {} }))).toBe(false);
    expect(shouldProxy(stream({ headers: { Referer: "  " } }))).toBe(false);
    expect(playbackUrl(stream())).toBe("https://cdn.example/show/ep1.mp4");
  });

  it("keeps HLS direct even when headers are present", () => {
    const hls = stream({
      url: "https://cdn.example/show/master.m3u8",
      type: "hls",
      headers: { Referer: "https://source.example/" },
    });
    expect(isMp4Stream(hls)).toBe(false);
    expect(shouldProxy(hls)).toBe(false);
    expect(playbackUrl(hls)).toBe(hls.url);
  });

  it("prefers the proxied URL when one is set", () => {
    const proxied = stream({
      headers: { Referer: "https://source.example/" },
      proxiedUrl: "http://127.0.0.1:39321/s/abc/file?token=def",
      proxyId: "abc",
    });
    expect(shouldProxy(proxied)).toBe(true);
    expect(playbackUrl(proxied)).toBe(proxied.proxiedUrl);
    expect(playbackUrl(stream())).toBe("https://cdn.example/show/ep1.mp4");
  });
});

describe("proxyFlagEnabled", () => {
  it("is off by default", () => {
    expect(proxyFlagEnabled(null)).toBe(false);
    expect(proxyFlagEnabled({ getItem: () => null })).toBe(false);
  });

  it("reads the local flag", () => {
    expect(proxyFlagEnabled({ getItem: () => "true" })).toBe(true);
    expect(proxyFlagEnabled({ getItem: () => "1" })).toBe(true);
    expect(proxyFlagEnabled({ getItem: () => "false" })).toBe(false);
  });
});
