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

  it("redacts refresh tokens and set-cookie header values", () => {
    const query = redactSecrets(
      "https://cdn.example/v?refresh_token=refresh123&client_secret=client456&lang=en",
    );
    expect(query).toContain("refresh_token=[redacted]");
    expect(query).toContain("client_secret=[redacted]");
    expect(query).toContain("lang=en");
    expect(query).not.toContain("refresh123");
    expect(query).not.toContain("client456");

    const setCookie = redactSecrets("Set-Cookie: sid=abc123; Path=/; HttpOnly");
    expect(setCookie).not.toContain("abc123");
    expect(setCookie).toContain("[redacted]");
  });

  it("redacts an inline Authorization header to the end of the line", () => {
    const out = redactSecrets("request failed with Authorization: Bearer secret123 and retry later");
    expect(out).not.toContain("secret123");
    expect(out).toContain("[redacted]");
  });

  it("redacts a bare token assignment outside a URL", () => {
    const out = redactSecrets("boot failed token=secret123 lang=en");
    expect(out).toContain("token=[redacted]");
    expect(out).toContain("lang=en");
    expect(out).not.toContain("secret123");
  });

  it("leaves a clean string alone", () => {
    expect(redactSecrets("mpv not found")).toBe("mpv not found");
  });
});
