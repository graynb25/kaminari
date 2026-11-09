import { describe, expect, it } from "vitest";
import {
  LIBMPV_FLAG_KEY,
  libmpvEnabled,
  libmpvFlagEnabled,
  libmpvNeedsProxy,
  libmpvUrlEnabled,
  setLibmpvFlag,
} from "./libmpv";

describe("libmpvFlagEnabled", () => {
  it("is off by default", () => {
    expect(libmpvFlagEnabled(null)).toBe(false);
    expect(libmpvFlagEnabled({ getItem: () => null })).toBe(false);
  });

  it("reads the local flag", () => {
    expect(libmpvFlagEnabled({ getItem: () => "true" })).toBe(true);
    expect(libmpvFlagEnabled({ getItem: () => "1" })).toBe(true);
    expect(libmpvFlagEnabled({ getItem: () => "false" })).toBe(false);
  });
});

describe("libmpvNeedsProxy", () => {
  it("is false without headers", () => {
    expect(libmpvNeedsProxy({})).toBe(false);
    expect(libmpvNeedsProxy({ headers: null })).toBe(false);
    expect(libmpvNeedsProxy({ headers: {} })).toBe(false);
    expect(libmpvNeedsProxy({ headers: { Referer: "  " } })).toBe(false);
  });

  it("is true when headers have no proxied URL", () => {
    expect(
      libmpvNeedsProxy({ headers: { Referer: "https://source.example/" } }),
    ).toBe(true);
    expect(
      libmpvNeedsProxy({
        headers: { Referer: "https://source.example/" },
        proxiedUrl: "   ",
      }),
    ).toBe(true);
  });

  it("is false once the proxy has a URL", () => {
    expect(
      libmpvNeedsProxy({
        headers: { Referer: "https://source.example/" },
        proxiedUrl: "http://127.0.0.1:39321/s/abc/file?token=def",
      }),
    ).toBe(false);
  });
});

describe("libmpvUrlEnabled", () => {
  it("only turns on with the exact flag", () => {
    expect(libmpvUrlEnabled("?libmpv=1")).toBe(true);
    expect(libmpvUrlEnabled("?libmpv=true")).toBe(false);
    expect(libmpvUrlEnabled("?mpvHarness=1")).toBe(false);
    expect(libmpvUrlEnabled("")).toBe(false);
  });
});

describe("setLibmpvFlag", () => {
  it("writes the shared key and reads it back", () => {
    const store: Record<string, string> = {};
    const storage = { setItem: (key: string, value: string) => (store[key] = value) };

    setLibmpvFlag(true, storage);
    expect(store[LIBMPV_FLAG_KEY]).toBe("true");
    expect(libmpvFlagEnabled({ getItem: (key) => store[key] ?? null })).toBe(true);

    setLibmpvFlag(false, storage);
    expect(libmpvFlagEnabled({ getItem: (key) => store[key] ?? null })).toBe(false);
  });

  it("stays quiet when storage is blocked", () => {
    expect(() => setLibmpvFlag(true, null)).not.toThrow();
    expect(() =>
      setLibmpvFlag(true, {
        setItem: () => {
          throw new Error("blocked");
        },
      }),
    ).not.toThrow();
  });
});

describe("libmpvEnabled", () => {
  it("combines the stored flag with the URL flag", () => {
    // No browser globals in the test runner, so the storage path stays off.
    expect(libmpvEnabled()).toBe(false);
  });
});
