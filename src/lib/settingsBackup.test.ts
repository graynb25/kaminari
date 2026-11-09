import { describe, expect, it } from "vitest";
import { parseSettings } from "./settingsBackup";

function settings(overrides: Record<string, unknown> = {}): string {
  return JSON.stringify({
    format: "kaminari-settings",
    version: 1,
    appVersion: "0.20.0",
    exportedAt: "2026-11-09T10:00:00.000Z",
    theme: "dark",
    accent: "ocean",
    proxy: true,
    ...overrides,
  });
}

describe("parseSettings", () => {
  it("reads a valid settings file", () => {
    const data = parseSettings(settings());
    expect(data.format).toBe("kaminari-settings");
    expect(data.version).toBe(1);
    expect(data.appVersion).toBe("0.20.0");
    expect(data.theme).toBe("dark");
    expect(data.accent).toBe("ocean");
    expect(data.proxy).toBe(true);
  });

  it("rejects invalid JSON", () => {
    expect(() => parseSettings("{not json")).toThrow(/not valid JSON/);
  });

  it("rejects a wrong format or version", () => {
    expect(() => parseSettings(settings({ format: "other" }))).toThrow(/not a Kaminari/);
    expect(() => parseSettings(settings({ version: 2 }))).toThrow(/Unsupported settings version/);
  });

  it("rejects an invalid theme or proxy flag", () => {
    expect(() => parseSettings(settings({ theme: "midnight" }))).toThrow(/invalid theme/);
    expect(() => parseSettings(settings({ proxy: "yes" }))).toThrow(/invalid proxy/);
  });

  it("falls back to violet for an unknown accent", () => {
    expect(parseSettings(settings({ accent: "teal" })).accent).toBe("violet");
    expect(parseSettings(settings({ accent: 7 })).accent).toBe("violet");
  });
});
