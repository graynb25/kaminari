import { describe, expect, it } from "vitest";
import { parseAccent } from "./accent";

describe("parseAccent", () => {
  it("accepts a known preset", () => {
    expect(parseAccent("ocean")).toBe("ocean");
    expect(parseAccent("rose")).toBe("rose");
  });

  it("falls back to violet for an unknown or missing value", () => {
    expect(parseAccent("teal")).toBe("violet");
    expect(parseAccent(null)).toBe("violet");
  });
});
