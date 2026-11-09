import { describe, expect, it } from "vitest";
import {
  isHlsStream,
  isMp4Stream,
  playbackUrl,
  PROXY_FLAG_KEY,
  proxyFlagEnabled,
  setProxyFlag,
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

  it("proxies HLS streams with headers", () => {
    const hls = stream({
      url: "https://cdn.example/show/master.m3u8",
      type: "hls",
      headers: { Referer: "https://source.example/" },
    });
    expect(isHlsStream(hls)).toBe(true);
    expect(isMp4Stream(hls)).toBe(false);
    expect(shouldProxy(hls)).toBe(true);
  });

  it("leaves DASH and embed streams with headers off the proxy", () => {
    const dash = stream({
      url: "https://cdn.example/show/manifest.mpd",
      type: "dash",
      headers: { Referer: "https://source.example/" },
    });
    expect(isHlsStream(dash)).toBe(false);
    expect(isMp4Stream(dash)).toBe(false);
    expect(shouldProxy(dash)).toBe(false);

    const embed = stream({
      url: "https://cdn.example/show/embed",
      type: "embed",
      headers: { Referer: "https://source.example/" },
    });
    expect(shouldProxy(embed)).toBe(false);
  });

  it("leaves headerless HLS on the direct path", () => {
    const hls = stream({ url: "https://cdn.example/show/index.m3u8", type: "hls" });
    expect(shouldProxy(hls)).toBe(false);
    expect(playbackUrl(hls)).toBe(hls.url);
  });

  it("detects HLS from the URL when the type is missing", () => {
    expect(isHlsStream(stream({ url: "https://cdn.example/show/index.m3u8?token=1", type: "" }))).toBe(
      true,
    );
    expect(isHlsStream(stream({ url: "https://cdn.example/show/ep1.mp4", type: "" }))).toBe(false);
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

    const proxiedHls = stream({
      url: "https://cdn.example/show/index.m3u8",
      type: "hls",
      headers: { Referer: "https://source.example/" },
      proxiedUrl: "http://127.0.0.1:39321/s/abc/manifest.m3u8?token=def",
      proxyId: "abc",
    });
    expect(playbackUrl(proxiedHls)).toBe(proxiedHls.proxiedUrl);
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

describe("setProxyFlag", () => {
  it("writes the shared key and reads it back", () => {
    const store: Record<string, string> = {};
    const storage = { setItem: (key: string, value: string) => (store[key] = value) };

    setProxyFlag(true, storage);
    expect(store[PROXY_FLAG_KEY]).toBe("true");
    expect(proxyFlagEnabled({ getItem: (key) => store[key] ?? null })).toBe(true);

    setProxyFlag(false, storage);
    expect(store[PROXY_FLAG_KEY]).toBe("false");
    expect(proxyFlagEnabled({ getItem: (key) => store[key] ?? null })).toBe(false);
  });

  it("stays quiet when storage is blocked", () => {
    expect(() => setProxyFlag(true, null)).not.toThrow();
    expect(() =>
      setProxyFlag(true, {
        setItem: () => {
          throw new Error("blocked");
        },
      }),
    ).not.toThrow();
  });
});
