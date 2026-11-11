import { describe, expect, it } from "vitest";
import {
  HISTORY_RETENTION_KEY,
  historyRetentionDays,
  parseHistoryRetention,
  readHistoryRetention,
  saveHistoryRetention,
} from "./historyRetention";

describe("history retention", () => {
  it("reads the stored window and falls back to Forever", () => {
    expect(readHistoryRetention({ getItem: () => "30" })).toBe("30");
    expect(readHistoryRetention({ getItem: () => "365" })).toBe("365");
    expect(readHistoryRetention({ getItem: () => "forever" })).toBe("forever");
    expect(readHistoryRetention({ getItem: () => "7" })).toBe("forever");
    expect(readHistoryRetention({ getItem: () => null })).toBe("forever");
  });

  it("saves under the settings key", () => {
    const saved: Record<string, string> = {};
    saveHistoryRetention("90", {
      setItem: (key, value) => {
        saved[key] = value;
      },
    });
    expect(saved[HISTORY_RETENTION_KEY]).toBe("90");
  });

  it("maps Forever to no cutoff and the other options to days", () => {
    expect(historyRetentionDays("forever")).toBeNull();
    expect(historyRetentionDays("30")).toBe(30);
    expect(historyRetentionDays("90")).toBe(90);
    expect(historyRetentionDays("365")).toBe(365);
  });

  it("rejects unknown values instead of inventing a window", () => {
    expect(parseHistoryRetention("0")).toBe("forever");
    expect(parseHistoryRetention(30)).toBe("forever");
    expect(parseHistoryRetention(undefined)).toBe("forever");
  });
});
