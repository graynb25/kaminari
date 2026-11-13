import { describe, expect, it } from "vitest";
import type { AiringScheduleEntry } from "./types";
import { dayKey, groupByDay, laterEntries, nextDayWithEntries, rollingDays } from "./scheduleDays";

const YORK = "America/New_York";
const TOKYO = "Asia/Tokyo";

function entry(anilistId: number, airingAt: number, episode = 1): AiringScheduleEntry {
  return { anilistId, episode, airingAt };
}

describe("dayKey", () => {
  it("keeps DST entries on March 8 in New York", () => {
    const early = Math.floor(Date.UTC(2026, 2, 8, 6, 30, 0) / 1000);
    const late = Math.floor(Date.UTC(2026, 2, 8, 7, 30, 0) / 1000);
    expect(dayKey(early, YORK)).toBe("2026-03-08");
    expect(dayKey(late, YORK)).toBe("2026-03-08");
  });

  it("keeps March 7 night on March 7", () => {
    const night = Math.floor(Date.UTC(2026, 2, 8, 4, 30, 0) / 1000);
    expect(dayKey(night, YORK)).toBe("2026-03-07");
  });
});

describe("rollingDays", () => {
  it("returns seven distinct days across the DST jump", () => {
    const now = Math.floor(Date.UTC(2026, 2, 7, 17, 0, 0) / 1000);
    const days = rollingDays(now, YORK, 7);
    expect(days.map((day) => day.key)).toEqual([
      "2026-03-07",
      "2026-03-08",
      "2026-03-09",
      "2026-03-10",
      "2026-03-11",
      "2026-03-12",
      "2026-03-13",
    ]);
    expect(new Set(days.map((day) => day.key)).size).toBe(7);
    expect(days[0].isToday).toBe(true);
    expect(days.slice(1).every((day) => !day.isToday)).toBe(true);
    expect(days.every((day) => day.entries.length === 0)).toBe(true);
  });
});

describe("groupByDay", () => {
  it("splits entries across local midnight in Tokyo", () => {
    const now = Math.floor(Date.UTC(2026, 10, 14, 12, 0, 0) / 1000);
    const days = rollingDays(now, TOKYO, 7);
    const before = Math.floor(Date.UTC(2026, 10, 14, 14, 59, 0) / 1000);
    const after = Math.floor(Date.UTC(2026, 10, 14, 15, 1, 0) / 1000);
    expect(dayKey(before, TOKYO)).not.toBe(dayKey(after, TOKYO));
    const grouped = groupByDay([entry(1, before), entry(2, after)], days, TOKYO);
    const withBefore = grouped.find((day) => day.key === dayKey(before, TOKYO));
    const withAfter = grouped.find((day) => day.key === dayKey(after, TOKYO));
    expect(withBefore?.entries.map((item) => item.anilistId)).toEqual([1]);
    expect(withAfter?.entries.map((item) => item.anilistId)).toEqual([2]);
  });

  it("sorts entries within a day and drops outside days", () => {
    const now = Math.floor(Date.UTC(2026, 10, 14, 12, 0, 0) / 1000);
    const days = rollingDays(now, "UTC", 7);
    const first = days[1].key;
    const late = entry(1, Math.floor(new Date(`${first}T22:00:00Z`).getTime() / 1000));
    const early = entry(2, Math.floor(new Date(`${first}T08:00:00Z`).getTime() / 1000));
    const outside = entry(3, now + 30 * 86400);
    const grouped = groupByDay([late, early, outside], days, "UTC");
    const target = grouped.find((day) => day.key === first);
    expect(target?.entries.map((item) => item.anilistId)).toEqual([2, 1]);
    expect(grouped.flatMap((day) => day.entries).some((item) => item.anilistId === 3)).toBe(false);
  });
});

describe("nextDayWithEntries", () => {
  it("prefers today when today has entries", () => {
    const now = Math.floor(Date.UTC(2026, 10, 14, 12, 0, 0) / 1000);
    const days = rollingDays(now, "UTC", 7).map((day, i) => ({
      ...day,
      entries: i === 0 ? [entry(1, now + 3600)] : [],
    }));
    expect(nextDayWithEntries(days, days[0].key)).toBe(days[0].key);
  });

  it("moves forward when today is empty", () => {
    const now = Math.floor(Date.UTC(2026, 10, 14, 12, 0, 0) / 1000);
    const days = rollingDays(now, "UTC", 7).map((day, i) => ({
      ...day,
      entries: i === 2 ? [entry(1, now + 3600)] : [],
    }));
    expect(nextDayWithEntries(days, days[0].key)).toBe(days[2].key);
  });

  it("moves past an empty selected day and wraps", () => {
    const now = Math.floor(Date.UTC(2026, 10, 14, 12, 0, 0) / 1000);
    const days = rollingDays(now, "UTC", 7).map((day, i) => ({
      ...day,
      entries: i === 0 ? [entry(1, now + 3600)] : [],
    }));
    expect(nextDayWithEntries(days, days[3].key)).toBe(days[0].key);
  });

  it("returns null when nothing airs", () => {
    const now = Math.floor(Date.UTC(2026, 10, 14, 12, 0, 0) / 1000);
    const days = rollingDays(now, "UTC", 7);
    expect(nextDayWithEntries(days, days[0].key)).toBeNull();
  });
});

describe("laterEntries", () => {
  it("keeps only future entries outside the week", () => {
    const now = Math.floor(Date.UTC(2026, 10, 14, 12, 0, 0) / 1000);
    const days = rollingDays(now, "UTC", 7);
    const inside = entry(1, now + 3600);
    const past = entry(2, now - 3600);
    const farFuture = entry(3, now + 10 * 86400);
    const farPastWindow = entry(4, now - 10 * 86400);
    const result = laterEntries([inside, past, farFuture, farPastWindow], days, now, "UTC");
    expect(result.map((item) => item.anilistId)).toEqual([3]);
  });

  it("sorts later entries by airing time", () => {
    const now = Math.floor(Date.UTC(2026, 10, 14, 12, 0, 0) / 1000);
    const days = rollingDays(now, "UTC", 7);
    const b = entry(2, now + 12 * 86400);
    const a = entry(1, now + 10 * 86400);
    expect(laterEntries([b, a], days, now, "UTC").map((item) => item.anilistId)).toEqual([1, 2]);
  });
});
