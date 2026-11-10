import { describe, expect, it } from "vitest";
import { autoplayEnabled, setAutoplayEnabled } from "./autoplay";

describe("autoplay preference", () => {
  it("defaults to on", () => {
    expect(autoplayEnabled({ getItem: () => null })).toBe(true);
    expect(autoplayEnabled(null)).toBe(true);
  });

  it("reads off and on values", () => {
    expect(autoplayEnabled({ getItem: () => "false" })).toBe(false);
    expect(autoplayEnabled({ getItem: () => "0" })).toBe(false);
    expect(autoplayEnabled({ getItem: () => "true" })).toBe(true);
    expect(autoplayEnabled({ getItem: () => "1" })).toBe(true);
  });

  it("stores the choice", () => {
    let saved = "";
    setAutoplayEnabled(false, { setItem: (_key, value) => (saved = value) });
    expect(saved).toBe("false");
    setAutoplayEnabled(true, { setItem: (_key, value) => (saved = value) });
    expect(saved).toBe("true");
  });
});
