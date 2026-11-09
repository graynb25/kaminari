import { describe, expect, it } from "vitest";
import { redactSecrets } from "./redact";

describe("redactSecrets", () => {
  it("redacts query token values but keeps safe params", () => {
    const out = redactSecrets("https://cdn.example/v?token=secret123&lang=en");
    expect(out).toContain("token=[redacted]");
    expect(out).toContain("lang=en");
    expect(out).not.toContain("secret123");
  });

  it("redacts Authorization and Cookie header values", () => {
    const auth = redactSecrets("Authorization: Bearer secret123");
    expect(auth).not.toContain("secret123");
    expect(auth).toContain("[redacted]");

    const cookie = redactSecrets("Cookie: session=abc123; theme=dark");
    expect(cookie).not.toContain("abc123");
    expect(cookie).not.toContain("theme=dark");
    expect(cookie).toContain("[redacted]");
  });

  it("leaves a clean string alone", () => {
    expect(redactSecrets("mpv not found")).toBe("mpv not found");
  });
});
